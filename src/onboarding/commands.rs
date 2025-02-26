use anyhow::Result;
use serde_json::json;

use crate::onboarding::util::prompt_text;

pub fn command_questions(context: &mut tera::Context) -> Result<()> {
    println!(
        "\nKwaak agents can run tests and use code coverage when coding. Kwaak uses tests as an extra feedback moment for agents"
    );

    let test_command =
        prompt_text("Test command (optional, <esc> to skip)", None).prompt_skippable()?;

    let coverage_command =
        prompt_text("Coverage command (optional, <esc> to skip)", None).prompt_skippable()?;

    context.insert(
        "commands",
        &json!({
            "test": test_command,
            "coverage": coverage_command,
        }),
    );

    Ok(())
}
