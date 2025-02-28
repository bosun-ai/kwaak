use ratatui::prelude::*;

use crate::{
    chat::Chat,
    chat_message::{ChatMessage, ChatRole},
};

mod message_styles {
    use super::{Color, Modifier, Style};

    pub const USER: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::ITALIC);

    pub const ASSISTANT: Style = Style::new()
        .fg(Color::Rgb(200, 160, 255))
        .add_modifier(Modifier::BOLD);

    pub const SYSTEM: Style = Style::new().add_modifier(Modifier::DIM);

    pub const TOOL_DONE: Style = Style::new().fg(Color::Green).add_modifier(Modifier::DIM);

    pub const TOOL_CALLED: Style = Style::new().add_modifier(Modifier::DIM);

    pub const COMMAND: Style = Style::new()
        .fg(Color::LightMagenta)
        .add_modifier(Modifier::BOLD);
}
#[allow(clippy::trivially_copy_pass_by_ref)]
pub fn get_style_and_prefix(role: &ChatRole) -> (&'static str, Style) {
    match role {
        ChatRole::User => ("▶ ", message_styles::USER),
        ChatRole::Assistant => ("✦ ", message_styles::ASSISTANT),
        ChatRole::System => ("ℹ ", message_styles::SYSTEM),
        ChatRole::Tool => ("⚙ ", message_styles::TOOL_DONE), // unused
        ChatRole::Command => ("» ", message_styles::COMMAND),
    }
}

// TODO: Maybe have tool state just on the message?
pub fn format_chat_message<'a>(current_chat: &Chat, message: &'a ChatMessage) -> Text<'a> {
    // TODO: Use this as a cache
    if let Some(rendered) = message.rendered() {
        return rendered.to_owned();
    }
    let (prefix, style) = get_style_and_prefix(message.role());

    // Render markdown first
    let mut rendered_text = tui_markdown::from_str(message.content());

    // Prepend the styled prefix to the first line
    if let Some(first_line) = rendered_text.lines.first_mut() {
        first_line.spans.insert(0, Span::styled(prefix, style));
    }

    // Remove background color from all lines
    // tui_markdown adds background colors and underlines to some headers
    rendered_text.lines.iter_mut().for_each(|line| {
        line.spans.iter_mut().for_each(|span| {
            span.style = span
                .style
                .bg(Color::Reset)
                .remove_modifier(Modifier::UNDERLINED);
        });
    });

    if let Some(swiftide::chat_completion::ChatMessage::Assistant(.., Some(tool_calls))) =
        message.original()
    {
        if !message.content().is_empty() {
            rendered_text.push_line(Line::from("\n\n"));
        }
        for tool_call in tool_calls {
            if tool_call.name() == "stop" {
                continue;
            }
            let is_done = current_chat.is_tool_call_completed(tool_call.id());
            let tool_call_text = format_tool_call(tool_call);
            let tool_prefix = "⚙ ";

            if is_done {
                // add a suffix checkmark on the end
                let checkmark = " ✓";
                rendered_text.lines.push(Line::styled(
                    [tool_prefix, &tool_call_text, checkmark].join(" "),
                    message_styles::TOOL_DONE,
                ));
            } else {
                rendered_text.lines.push(Line::styled(
                    [tool_prefix, &tool_call_text].join(" "),
                    message_styles::TOOL_CALLED,
                ));
            }
        }
    }

    // If it's a single line message and not an assistant message, apply the style to the whole line
    if !message.role().is_assistant() && rendered_text.lines.len() == 1 {
        for line in &mut rendered_text.lines {
            for span in &mut line.spans {
                span.style = style;
            }
        }
    }

    rendered_text
}

fn format_tool_call(tool_call: &swiftide::chat_completion::ToolCall) -> String {
    if let Some(formatted) = pretty_format_tool(tool_call) {
        return formatted;
    }

    // If args, parse them as a json value, then if its just one, render only the value, otherwise
    // limit the output to 20 characters
    let formatted_args = tool_call.args().and_then(|args| {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(args) {
            if let Some(obj) = parsed.as_object() {
                if obj.is_empty() {
                    return None;
                }
                if obj.keys().count() == 1 {
                    let key = obj.keys().next().unwrap();
                    let val = obj[key].as_str().unwrap_or_default();

                    if val.len() > 20 {
                        return Some(format!("{} ...", &val[..20]));
                    }

                    return Some(val.to_string());
                }
                if args.len() > 20 {
                    return Some(format!("{} ...", &args[..20]));
                }

                return Some(args.to_string());
            }
            None
        } else {
            None
        }
    });

    if let Some(args) = formatted_args {
        format!("calling tool `{}` with `{}`", tool_call.name(), args)
    } else {
        format!("calling tool `{}`", tool_call.name())
    }
}

fn pretty_format_tool(tool_call: &swiftide::chat_completion::ToolCall) -> Option<String> {
    let parsed_lt = tool_call
        .args()
        .and_then(|args| serde_json::from_str::<serde_json::Value>(args).ok());

    let parsed_args = parsed_lt.as_ref().and_then(serde_json::Value::as_object);

    // TODO: Would be nice to have user friendly result stats here
    Some(match tool_call.name() {
        "shell_command" => format!("running shell command `{}`", get_value(parsed_args, "cmd")?),
        "read_file" => format!("reading file `{}`", get_value(parsed_args, "file_name")?),
        "write_file" => format!("writing file `{}`", get_value(parsed_args, "file_name")?),
        "search_file" => format!(
            "searching for files matching `{}`",
            get_value(parsed_args, "file_name")?
        ),
        "search_code" => format!(
            "searching for code matching `{}`",
            get_value(parsed_args, "query")?
        ),
        "git" => format!(
            "running git command `{}`",
            get_value(parsed_args, "command")?
        ),
        "explain_code" => format!(
            "querying for code explaining `{}`",
            get_value(parsed_args, "query")?
        ),
        "create_or_update_pull_request" => "creating a pull request".to_string(),
        "run_tests" => "running tests".to_string(),
        "run_coverage" => "running tests and gathering coverage".to_string(),
        "search_web" => format!(
            "searching the web for `{}`",
            get_value(parsed_args, "query")?
        ),
        "github_search_code" => format!(
            "searching github for code matching `{}`",
            get_value(parsed_args, "query")?
        ),
        "replace_lines" => format!(
            "replacing lines in file `{}`",
            get_value(parsed_args, "file_name")?
        ),
        "add_lines" => format!(
            "adding lines to file `{}`",
            get_value(parsed_args, "file_name")?
        ),
        "read_file_with_line_numbers" => format!(
            "reading file `{}` (with line numbers)",
            get_value(parsed_args, "file_name")?
        ),
        "fetch_url" => format!("fetching url `{}`", get_value(parsed_args, "url")?),
        "delegate_coding_agent" => "delegating task to coding agent".into(),
        _ => return None,
    })
}

fn get_value<'a>(
    args: Option<&'a serde_json::Map<String, serde_json::Value>>,
    key: &str,
) -> Option<&'a str> {
    args?.get(key).and_then(serde_json::Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_message::ChatRole;
    use ratatui::text::Span;

    #[test]
    fn test_get_style_and_prefix() {
        assert_eq!(
            get_style_and_prefix(&ChatRole::User),
            ("▶ ", message_styles::USER)
        );
        assert_eq!(
            get_style_and_prefix(&ChatRole::Assistant),
            ("✦ ", message_styles::ASSISTANT)
        );
        assert_eq!(
            get_style_and_prefix(&ChatRole::System),
            ("ℹ ", message_styles::SYSTEM)
        );
        assert_eq!(
            get_style_and_prefix(&ChatRole::Command),
            ("» ", message_styles::COMMAND)
        );
    }

    #[test]
    fn test_format_chat_message() {
        let chat = Chat::default();
        let message = ChatMessage::new_user("Hello, world!");

        let formatted_message = format_chat_message(&chat, &message);

        assert_eq!(formatted_message.lines.len(), 1);
        assert_eq!(
            formatted_message.lines[0].spans,
            vec![
                Span::styled("▶ ", message_styles::USER),
                Span::styled("Hello, world!", message_styles::USER)
            ]
        );
    }

    #[test]
    fn test_format_tool_call() {
        let tool_call = swiftide::chat_completion::ToolCall::builder()
            .name("shell_command")
            .id("tool_id")
            .args("{\"cmd\":\"ls\"}")
            .build()
            .unwrap();
        let formatted_tool_call = format_tool_call(&tool_call);

        assert_eq!(formatted_tool_call, "running shell command `ls`");
    }

    #[test]
    fn test_get_value() {
        let args = serde_json::json!({"key": "value"}).as_object().cloned();
        let value = get_value(args.as_ref(), "key");

        assert_eq!(value, Some("value"));
    }
}
