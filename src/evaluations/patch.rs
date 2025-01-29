//! The patch module is meant to reveal problems in agents when making modifications to the source code. Specifically
//! in large files and/or files with semantic whitespace.

use crate::agent::tools;
use crate::config::Config;
use swiftide::chat_completion::Tool;
use crate::evaluations::{start_tool_evaluation_agent, output::EvalOutput, logging_responder::LoggingResponder};
use crate::repository::Repository;
use anyhow::Result;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use uuid::Uuid;

const EXPECTED_REMOVALS: &[&str] = &[
    "            self._content_consumed = True",
];

const EXPECTED_ADDITIONS: &[&str] = &[
    "                except socket.error as e:",
    "                    raise ConnectionError(e)",
    "            finally:",
    "                self._content_consumed = True",
];

/// The prompt to give to the agent when making the changes
/// 
/// The goal of the prompt is to get the agent to use its tools to patch the file without spending too much tokens
/// on exploring the context.
fn prompt() -> String {
    indoc::indoc! {"
        There is a bug in the `src/evaluations/fixtures/swebench_2148/models.py` file in the `iter_content` method.

        To fix it add an additional exception handler to the nested try block that looks like this (but adjusted for indentation):

        ```
        except socket.error as e:
            raise ConnectionError(e)
        ```

        And also add a finally clause to the outer try block that looks like this (but adjusted for indentation):

        ```
        finally:
            self._content_consumed = True
        ```

        Apply only these fixes, do not make any other changes to the code. The file is long and the modifications
        are small.
    "}.to_string()
}

async fn reset_file() -> Result<()> {
    // Run git checkout to reset the file
    let status = Command::new("git")
        .args(&[
            "checkout",
            "HEAD",
            "--",
            "src/evaluations/fixtures/swebench_2148/models.py",
        ])
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to reset file using git checkout");
    }
    Ok(())
}

async fn compare_changes(eval_output: &EvalOutput) -> Result<bool> {
    // Get the diff of the current changes
    let output = Command::new("git")
        .args(&[
            "diff",
            "--",
            "src/evaluations/fixtures/swebench_2148/models.py",
        ])
        .output()?;

    if !output.status.success() {
        anyhow::bail!("Failed to get git diff");
    }

    let diff = String::from_utf8(output.stdout)?;

    // Save the diff
    eval_output.write_diff(&diff)?;

    let changes_diff = diff
        .split_once("+++ b/src/evaluations/fixtures/swebench_2148/models.py")
        .expect("Could not split diff")
        .1;

    let mut success = true;
    let additions = changes_diff
        .lines()
        .filter(|s| s.starts_with('+'))
        // remove + from the start of each line
        .map(|s| s.trim_start_matches('+'))
        // remove additions that are empty or contain only whitespace
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>();

    let removals = changes_diff
        .lines()
        .filter(|s| s.starts_with('-'))
        // remove - from the start of each line
        .map(|s| s.trim_start_matches('-'))
        // remove removals that are empty or contain only whitespace
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>();

    let missing_removals: Vec<_> = EXPECTED_REMOVALS
        .iter()
        .filter(|r| !removals.contains(r))
        .collect();

    let missing_additions: Vec<_> = EXPECTED_ADDITIONS
        .iter()
        .filter(|a| !additions.contains(a))
        .collect();

    if !missing_removals.is_empty() {
        println!("Removals: [{removals:?}] do not contain expected removals");
        success = false;
    }

    if !missing_additions.is_empty() {
        println!("Additions: [{additions:?}] do not contain expected additions");
        success = false;
    }

    if !success {
        write_failure_info(eval_output, &missing_removals, &missing_additions, &removals, &additions)?;
    }

    println!("\nChange validation result: {success}");

    // Reset changes after validation
    Command::new("git")
        .arg("checkout")
        .arg("HEAD")
        .arg("--")
        .arg("src/evaluations/fixtures/swebench_2148/models.py")
        .output()?;

    Ok(success)
}

fn write_failure_info(
    eval_output: &EvalOutput,
    missing_removals: &[&&str],
    missing_additions: &[&&str],
    found_removals: &[&str],
    found_additions: &[&str],
) -> Result<()> {
    let mut content = String::new();
    content.push_str("Expected changes were not found in the patch.\n\n");
    
    content.push_str("Missing removals:\n");
    for removal in missing_removals {
        content.push_str(&format!("{}\n", removal));
    }
    content.push('\n');
    
    content.push_str("Missing additions:\n");
    for addition in missing_additions {
        content.push_str(&format!("{}\n", addition));
    }
    content.push('\n');
    
    content.push_str("Found removals:\n");
    for removal in found_removals {
        content.push_str(&format!("{}\n", removal));
    }
    content.push('\n');
    
    content.push_str("Found additions:\n");
    for addition in found_additions {
        content.push_str(&format!("{}\n", addition));
    }

    eval_output.write_file("failed", &content)?;
    Ok(())
}

pub fn get_evaluation_tools() -> Result<Vec<Box<dyn Tool>>> {
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(tools::read_file()),
        Box::new(tools::write_file()),
        Box::new(tools::read_file_with_line_numbers()),
        Box::new(tools::search_file()),
        Box::new(tools::replace_lines()),
        Box::new(tools::add_lines()),
    ];

    Ok(tools)
}

async fn run_single_evaluation(iteration: u32) -> Result<bool> {
    let eval_output = EvalOutput::new("patch", iteration)?;
    let responder = Arc::new(LoggingResponder::new());

    // Create a new agent
    let uuid = Uuid::new_v4();
    let config_path = Path::new("test-config.toml");
    let repository = Repository::from_config(Config::load(&config_path).expect("Failed to load config").fill_llm_api_keys()?);
    
    let tools = get_evaluation_tools()?;
    let agent = start_tool_evaluation_agent(uuid, &repository, &prompt(), responder.clone(), tools).await?;

    // Send the query and wait for completion
    agent.query(&prompt()).await?;
    agent.run().await?;

    // Save agent log
    eval_output.write_agent_log(&responder.get_log())?;

    // Compare the changes
    compare_changes(&eval_output).await
}

pub async fn evaluate(iterations: u32) -> Result<()> {
    let mut successes = 0;

    for i in 0..iterations {
        println!("Running patch evaluation iteration {}", i + 1);

        // Reset the file to its original state before each iteration
        reset_file().await?;

        match run_single_evaluation(i + 1).await {
            Ok(true) => {
                println!("Iteration {} succeeded", i + 1);
                successes += 1;
            }
            Ok(false) => println!(
                "Iteration {} failed - changes did not match expected patch",
                i + 1
            ),
            Err(e) => println!("Iteration {} failed with error: {}", i + 1, e),
        }
    }

    println!(
        "Evaluation complete: {}/{} iterations succeeded",
        successes, iterations
    );
    Ok(())
}
