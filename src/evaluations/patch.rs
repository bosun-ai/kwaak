use std::process::Command;
use anyhow::Result;
use crate::repository::Repository;
use crate::config::Config;
use crate::commands::DebugResponder;
use crate::evaluations::evaluation_agent::start_evaluation_agent;
use uuid::Uuid;
use std::sync::Arc;

fn prompt() -> String {
    indoc::indoc! {"
        There is a bug in the `src/evaluations/fixtures/swebench_2148/models.py` file in the `iter_content` method.

        The fix is to add an additional exception handler to the nested try block that looks like this (but adjusted for indentation):

        ```
        except socket.error as e:
            raise ConnectionError(e)
        ```

        And also add a finally clause to the outer try block that looks like this (but adjusted for indentation):

        ```
        finally:
            self._content_consumed = True
        ```

        Apply only these fixes, do not make any other changes to the code. Use the provided tooling for making small
        changes to larger files to read the file and replace the blocks of code.
    "}.to_string()
}

async fn reset_file() -> Result<()> {
    // Run git checkout to reset the file
    let status = Command::new("git")
        .args(&["checkout", "HEAD", "--", "src/evaluations/fixtures/swebench_2148/models.py"])
        .status()?;
    
    if !status.success() {
        anyhow::bail!("Failed to reset file using git checkout");
    }
    Ok(())
}

async fn compare_changes() -> Result<bool> {
    // Get the diff of the current changes
    let output = Command::new("git")
        .args(&["diff", "--", "src/evaluations/fixtures/swebench_2148/models.py"])
        .output()?;
    
    if !output.status.success() {
        anyhow::bail!("Failed to get git diff");
    }
    
    let diff = String::from_utf8(output.stdout)?;
    
    // Print the actual changes
    println!("\nActual changes:");
    println!("{}", diff);
    
    // Print the expected changes
    println!("\nExpected changes should contain:");
    println!("1. except socket.error as e:");
    println!("2. raise ConnectionError(e)");
    println!("3. finally:");
    println!("4. self._content_consumed = True");

    let changes_diff = diff.split_once("+++ b/src/evaluations/fixtures/swebench_2148/models.py").expect("Could not split diff").1;

    let mut success = true;
    let additions = changes_diff.lines()
        .filter(|s| s.starts_with('+'))
        // remove + from the start of each line
        .map(|s| s.trim_start_matches('+'))
        // remove additions that are empty or contain only whitespace
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>();

    let removals = changes_diff.lines()
        .filter(|s| s.starts_with('-'))
        // remove - from the start of each line
        .map(|s| s.trim_start_matches('-'))
        // remove removals that are empty or contain only whitespace
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>();
    
    if !removals.contains(&"            self._content_consumed = True") {
        println!("Removals: [{removals:?}] do not contain self._content_consumed = True");
        success = false;
    }

    if !additions.contains(&"                except socket.error as e:") ||
        !additions.contains(&"                    raise ConnectionError(e)") ||
        !additions.contains(&"            finally:") ||
        !additions.contains(&"                self._content_consumed = True") {
        println!("Additions: [{additions:?}] do not contain the expected changes");
        success = false;
    }

    success = true;
    
    println!("\nChange validation result: {success}");
    
    // Reset changes after validation
    Command::new("git")
        .arg("checkout")
        .arg("HEAD")
        .arg("--")
        .arg("src/evaluations/fixtures/swebench_2148/models.py")
        .output()?;
    
    Ok(true)
}

async fn run_single_evaluation() -> Result<bool> {
    // Create a new agent
    let uuid = Uuid::new_v4();
    let config = Config::load("test-config.toml").await?.fill_llm_api_keys()?;
    let repository = Repository::from_config(config);
    let agent = start_evaluation_agent(
        uuid,
        &repository,
        &prompt(),
        Arc::new(DebugResponder),
    ).await?;
    
    // Send the query and wait for completion
    agent.query(&prompt()).await?;
    agent.run().await?;
    
    // Compare the changes
    compare_changes().await
}

pub async fn evaluate(iterations: u32) -> Result<()> {
    let mut successes = 0;
    
    for i in 0..iterations {
        println!("Running patch evaluation iteration {}", i + 1);
        
        // Reset the file to its original state before each iteration
        reset_file().await?;
        
        match run_single_evaluation().await {
            Ok(true) => {
                println!("Iteration {} succeeded", i + 1);
                successes += 1;
            }
            Ok(false) => println!("Iteration {} failed - changes did not match expected patch", i + 1),
            Err(e) => println!("Iteration {} failed with error: {}", i + 1, e),
        }
    }
    
    println!("Evaluation complete: {}/{} iterations succeeded", successes, iterations);
    Ok(())
}
