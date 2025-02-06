use anyhow::Result;

pub fn prompt_text<'a>(prompt: &'a str, default: Option<&'a str>) -> inquire::Text<'a> {
    let mut prompt = inquire::Text::new(prompt);

    if let Some(default) = default {
        prompt = prompt.with_default(default);
    }

    prompt
}

pub fn prompt_api_key<'a>(prompt: &'a str, default: Option<&'a str>) -> inquire::Text<'a> {
    let mut prompt = inquire::Text::new(prompt).with_validator(|input: &str| {
        if input.starts_with("env:") || input.starts_with("file:") {
            Ok(inquire::validator::Validation::Valid)
        } else {
            Ok(inquire::validator::Validation::Invalid(
                "API keys must start with `env:` or `file:`".into(),
            ))
        }
    });

    if let Some(default) = default {
        prompt = prompt.with_default(default);
    }

    prompt
}

#[allow(clippy::needless_pass_by_value)]
pub fn prompt_select<T>(prompt: &str, options: Vec<T>, default: Option<T>) -> Result<String>
where
    T: std::fmt::Display + std::cmp::PartialEq + Clone,
{
    let mut prompt = inquire::Select::new(prompt, options.clone());

    if let Some(default) = default {
        debug_assert!(
            options.contains(&default),
            "{} is not in the list of options, valid: {}",
            default,
            options
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
        if let Some(idx) = options.iter().position(|l| l == &default) {
            prompt = prompt.with_starting_cursor(idx);
        }
    }

    Ok(prompt.prompt()?.to_string())
}
