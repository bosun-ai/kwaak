//! The patch module is meant to reveal problems in agents when making modifications to the source code. Specifically
//! in large files and/or files with semantic whitespace.

use crate::agent::tools;
use crate::config::Config;
use crate::evaluations::{
    logging_responder::LoggingResponder, output::EvalOutput, start_tool_evaluation_agent,
};
use crate::repository::Repository;
use anyhow::Result;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use swiftide::chat_completion::Tool;
use uuid::Uuid;

const EXPECTED_REMOVALS: &[&str] = &["            self._content_consumed = True"];

const EXPECTED_ADDITIONS: &[&str] = &[
    "                except socket.error as e:",
    "                    raise ConnectionError(e)",
    "            finally:",
    "                self._content_consumed = True",
];

/// The goal of the prompt is to get the agent to use its tools to patch the file without spending too much tokens
/// on exploring the context.
fn prompt(iteration: u32) -> String {

    indoc::formatdoc! {"
        This is iteration {iteration}.

        There is a bug in the `src/evaluations/fixtures/swebench_2148/models.py` file in the `iter_content` method.

        To fix it add an additional exception handler to the nested try block that looks like this (but adjusted for indentation):

        ```
        except socket.error as e:
            raise ConnectionError(e)
        ```

        And also move the content consumed setter to a new finally clause on the outer try block that looks like this (but adjusted for indentation):

        ```
        finally:
            self._content_consumed = True
        ```

        Apply only these fixes, do not make any other changes to the code. The file is long and the modifications
        are small.
    "}.to_string()
}

fn reset_file() -> Result<()> {
    let status = Command::new("git")
        .args([
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

fn compare_changes(eval_output: &EvalOutput) -> Result<bool> {
    let output = Command::new("git")
        .args([
            "diff",
            "--",
            "src/evaluations/fixtures/swebench_2148/models.py",
        ])
        .output()?;

    if !output.status.success() {
        anyhow::bail!("Failed to get git diff");
    }

    let diff = String::from_utf8(output.stdout)?;

    eval_output.write_diff(&diff)?;

    let mut success = true;

    let changes_diff = diff
        .split_once("+++ b/src/evaluations/fixtures/swebench_2148/models.py")
        .ok_or(anyhow::anyhow!("Failed to split diff"))?
        .1;

    let additions = changes_diff
        .lines()
        .filter(|s| s.starts_with('+'))
        .map(|s| s.trim_start_matches('+'))
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>();

    let removals = changes_diff
        .lines()
        .filter(|s| s.starts_with('-'))
        .map(|s| s.trim_start_matches('-'))
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
        success = false;
    }

    if !missing_additions.is_empty() {
        success = false;
    }

    if !success {
        write_failure_info(
            eval_output,
            &missing_removals,
            &missing_additions,
            &removals,
            &additions,
        )?;
    }

    println!("\nChange validation result: {success}");

    Command::new("git")
        .args([
            "checkout",
            "HEAD",
            "--",
            "src/evaluations/fixtures/swebench_2148/models.py",
        ])
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
        content.push_str(&format!("{removal}\n"));
    }
    content.push('\n');

    content.push_str("Missing additions:\n");
    for addition in missing_additions {
        content.push_str(&format!("{addition}\n"));
    }
    content.push('\n');

    content.push_str("Found removals:\n");
    for removal in found_removals {
        content.push_str(&format!("{removal}\n"));
    }
    content.push('\n');

    content.push_str("Found additions:\n");
    for addition in found_additions {
        content.push_str(&format!("{addition}\n"));
    }

    eval_output.write_file("failed", &content)?;
    Ok(())
}

pub fn get_evaluation_tools() -> Result<Vec<Box<dyn Tool>>> {
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(tools::search_file()),
        Box::new(tools::read_file()),
        Box::new(tools::write_file()),
        Box::new(tools::read_file_with_line_numbers()),
        Box::new(tools::replace_lines()),
    ];

    Ok(tools)
}

async fn run_single_evaluation(iteration: u32) -> Result<bool> {
    let eval_output = EvalOutput::new("patch", iteration)?;
    let responder = Arc::new(LoggingResponder::new());

    let config_path = Path::new("test-config.toml");
    let repository = Repository::from_config(
        Config::load(&config_path)
            .expect("Failed to load config")
            .fill_llm_api_keys()?,
    );

    let tools = get_evaluation_tools()?;
    let agent =
        start_tool_evaluation_agent(&repository, responder.clone(), tools).await?;

    agent.query(&prompt(iteration)).await?;
    agent.run().await?;

    eval_output.write_agent_log(&responder.get_log())?;

    compare_changes(&eval_output)
}

pub async fn evaluate(iterations: u32) -> Result<()> {
    let mut successes = 0;

    for i in 0..iterations {
        println!("Running patch evaluation iteration {}", i + 1);

        // Reset the file to its original state before each iteration
        reset_file()?;

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

    println!("Evaluation complete: {successes}/{iterations} iterations succeeded");
    Ok(())
}
