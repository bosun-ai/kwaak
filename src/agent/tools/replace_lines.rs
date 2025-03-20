/// Replace lines in a file. This tool is in beta and only has a ~80% success rate.
use swiftide::traits::CommandError;

use anyhow::Result;
use swiftide::{
    chat_completion::{errors::ToolError, ToolOutput},
    traits::{AgentContext, Command},
};
use swiftide_macros::tool;

const REPLACE_LINES_DESCRIPTION: &str = "Replace lines in a file.

You MUST read the file with line numbers first BEFORE EVERY EDIT.

After editing, you MUST read the file again to get the new line numbers.

Line numbers are 1-indexed, you can ONLY use the line numbers retrieved when reading the file.

Do not include the line numbers in the content.

You MUST include TWO existing lines BEFORE and AFTER the lines you want to replace in the content.
You MUST make sure that the start_line  and end_line number are equal to the line numbers BEFORE replacing content.

It is VITAL you follow these instructions correctly, as otherwise the code will break and your manager will be very upset.
";

// Another invalid pair of values would be old_content:

// ```
// def a:
//   pass

// def b:
// ```

// and new_content:
// ```
// def a:
//   return True

// def b:
//   pass
// ```

// because the region of modification in new_content includes line 6 where old_content goes only to line 5.
// ";
#[tool(
    description = REPLACE_LINES_DESCRIPTION,
    param(name = "file_name", description = "Full path of the file"),
    param(
        name = "start_line",
        description = "First line number of the content you want to replace."
    ),
    param(
        name = "end_line",
        description = "Last line number of the content of the region you want to replace"
    ),
    param(
        name = "content",
        description = "Content to replace the region with. Include TWO lines before and TWO lines after the replaced content. Do not include line numbers."
    )
)]
pub async fn replace_lines(
    context: &dyn AgentContext,
    file_name: &str,
    start_line: &str,
    end_line: &str,
    content: &str,
) -> Result<ToolOutput, ToolError> {
    let cmd = Command::ReadFile(file_name.into());

    let file_content = match context.exec_cmd(&cmd).await {
        Ok(output) => output.output,
        Err(CommandError::NonZeroExit(output, ..)) => {
            return Ok(output.into());
        }
        Err(e) => return Err(e.into()),
    };

    let lines_len = file_content.lines().count();

    let Ok(start_line) = start_line.parse::<usize>() else {
        return Ok("Invalid start line number, must be a valid number greater than 0".into());
    };

    let Ok(end_line) = end_line.parse::<usize>() else {
        return Ok("Invalid end line number, must be a valid number 0 or greater".into());
    };

    if start_line > lines_len || end_line > lines_len {
        return Ok(format!("Start or end line number is out of bounds ({start_line} - {end_line}, max: {lines_len})").into());
    }

    if end_line > 0 && start_line > end_line {
        return Ok("Start line number must be less than or equal to end line number".into());
    }

    if start_line == 0 {
        return Ok("Start line number must be greater than 0".into());
    }

    let new_file_content = replace_content(&file_content, start_line, end_line, &content);

    if let Err(err) = new_file_content {
        return Ok(ToolOutput::Text(err.to_string()));
    }

    let write_cmd = Command::WriteFile(file_name.into(), new_file_content.unwrap());
    context.exec_cmd(&write_cmd).await?;

    Ok(format!("Successfully replaced content in {file_name}. Before making new edits, you MUST read the file again, as the line numbers WILL have changed.").into())
}

fn replace_content(
    file_content: &str,
    start_line: usize,
    end_line: usize,
    content: &str,
) -> Result<String> {
    let lines = file_content.lines().collect::<Vec<_>>();
    let content_lines = content.lines().collect::<Vec<_>>();

    let first_line = lines[start_line - 1];
    let content_first_line = content_lines[0];

    if start_line > 1 && !first_line.contains(content_first_line) {
        anyhow::bail!(
            "The line on line number {start_line} reads: `{first_line}`, which does not match the first line of the content: `{content_first_line}`."
        );
    }

    let last_line = lines[end_line - 1];
    let content_last_line = content_lines[content_lines.len() - 1];

    if end_line < lines.len() && !last_line.contains(content_last_line) {
        anyhow::bail!(
            "The line on line number {end_line} reads: `{last_line}`, which does not match the last line of the content: `{content_last_line}`."
        );
    }

    let first_line_indentation_mismatch: usize = first_line.find(content_first_line).unwrap_or(0);

    let mut content = content.to_string();
    if first_line_indentation_mismatch > 0 {
        let indentation_char = first_line.chars().next().unwrap_or(' ').to_string();

        content = content
            .lines()
            .map(|line| {
                let mut new_line = line.to_string();
                if !new_line.is_empty() {
                    new_line
                        .insert_str(0, &indentation_char.repeat(first_line_indentation_mismatch));
                }
                new_line
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    let prefix = file_content
        .split('\n')
        .take(start_line - 1)
        .collect::<Vec<_>>();
    let suffix = file_content.split('\n').skip(end_line).collect::<Vec<_>>();

    let new_file_content = [prefix, content.lines().collect::<Vec<_>>(), suffix]
        .concat()
        .join("\n");

    Ok(new_file_content)
}
