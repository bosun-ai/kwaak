/// Replace lines in a file. This tool is in beta and only has a ~80% success rate.
use swiftide::traits::CommandError;

use anyhow::Result;
use swiftide::tool;
use swiftide::{
    chat_completion::{ToolOutput, errors::ToolError},
    traits::{AgentContext, Command},
};

const REPLACE_LINES_DESCRIPTION: &str = "Replace lines in a file.

You MUST read the file with line numbers first BEFORE EVERY EDIT.

After editing, you MUST read the file again to get the new line numbers.

Line numbers are 1-indexed, you do not need to subtract 1 for start_line and end_line.

Do not include the line numbers in the content.

You MUST include a couple of lines BEFORE and AFTER the lines you want to replace.

The first and last lines of the content MUST NOT be blank (expand accordingly).

For example when making a modification to the following file:

2|def a:
3|  pass
4|
5|def b:
6|  pass

And you want to change line 3 to return True.

Valid values:

start_line: 2
end_line: 5
content:
```
def a:
  return True

def b:
```

Valid because the region is expanded to include lines 2 and 5. Expanding to just 4 would not be enough as it is blank.

Example of invalid values:

start_line: 3
end_line: 3
content:
```
  return True
```

Invalid because the region is not expanded to include lines 2 and 5.
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
        description = "First line of the region that surrounds the modifications"
    ),
    param(
        name = "end_line",
        description = "Last line of the region that surrounds the modifications"
    ),
    param(
        name = "content",
        description = "Code to replace the region with, containing both the modifications and some lines before and after"
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

    let file_content = match context.executor().exec_cmd(&cmd).await {
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
    context.executor().exec_cmd(&write_cmd).await?;

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
