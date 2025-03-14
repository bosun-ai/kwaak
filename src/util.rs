use swiftide::traits::{CommandError, CommandOutput};

#[must_use]
pub fn strip_markdown_tags(text: &str) -> String {
    if text.starts_with("```markdown") && text.ends_with("```") {
        text[12..text.len() - 3].trim().to_string()
    } else {
        text.to_string()
    }
}

// TODO: Would be nice if this was a method on a custom result in swiftide
pub fn accept_non_zero_exit(
    result: Result<CommandOutput, CommandError>,
) -> Result<CommandOutput, CommandError> {
    match result {
        Ok(output) | Err(CommandError::NonZeroExit(output)) => Ok(output),
        Err(err) => Err(err),
    }
}

#[must_use]
pub fn is_git_branch_change(cmd: &str) -> bool {
    cmd.starts_with("git checkout") || cmd.starts_with("git switch")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_markdown_tags() {
        // Case: String with surrounding ```markdown tags
        let input = "```markdown\nThis is a test.\n```";
        let expected = "This is a test.";
        assert_eq!(strip_markdown_tags(input), expected);

        // Case: String without surrounding ```markdown tags
        let input = "This is a test.";
        let expected = "This is a test.";
        assert_eq!(strip_markdown_tags(input), expected);

        // Case: Empty string
        let input = "";
        let expected = "";
        assert_eq!(strip_markdown_tags(input), expected);

        // Case: String with only opening ```markdown tag
        let input = "```markdown\nThis is a test.";
        let expected = "```markdown\nThis is a test.";
        assert_eq!(strip_markdown_tags(input), expected);

        // Case: String with only closing ``` tag
        let input = "This is a test.\n```";
        let expected = "This is a test.\n```";
        assert_eq!(strip_markdown_tags(input), expected);

        // Case: Raw string
        let input = r"```markdown
        This is a test.
        ```";
        let expected = "This is a test.";
        assert_eq!(strip_markdown_tags(input), expected);
    }

    #[test]
    fn test_is_git_branch_change() {
        assert!(is_git_branch_change("git checkout main"));
        assert!(is_git_branch_change("git switch feature-branch"));
        assert!(!is_git_branch_change("git commit -m 'initial commit'"));
        assert!(!is_git_branch_change("git push origin main"));
        assert!(!is_git_branch_change("checkout main"));
        assert!(!is_git_branch_change("switch feature-branch"));
    }
}
