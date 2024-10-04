pub fn strip_markdown_tags(text: &str) -> String {
    if text.starts_with("```markdown") && text.ends_with("```") {
        text[12..text.len() - 3].trim().to_string()
    } else {
        text.to_string()
    }
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
        let input = r#"```markdown
        This is a test.
        ```"#;
        let expected = "This is a test.";
        assert_eq!(strip_markdown_tags(input), expected);
    }
}
