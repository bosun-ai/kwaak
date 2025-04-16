use anyhow::Context as _;
use anyhow::Result;
use rust_embed::Embed;
use swiftide::prompt::Prompt;
use tera::Tera;

#[derive(Embed)]
#[folder = "templates"]
pub struct Templates;

// TODO: Figure out if we can extend the tera template repository in swiftide templates with
// /templates so that we can remove this
impl Templates {
    pub fn render(name: &str, context: &tera::Context) -> Result<String> {
        let byte_file =
            Templates::get(name).with_context(|| format!("Expected template {name}"))?;
        let template = std::str::from_utf8(&byte_file.data)?;

        Tera::one_off(template, context, false).context("Failed to render template")
    }

    pub fn from_file(name: &str) -> Result<String> {
        let byte_file =
            Templates::get(name).with_context(|| format!("Expected template {name}"))?;

        String::from_utf8(byte_file.data.into_owned()).context("Failed to read template")
    }

    /// Load the template as a swiftide prompt
    ///
    /// Prefer this over the other methods, they'll be removed soon.
    /// Intenrally Templates are a light wrapper around Tera with some shenanigans
    pub fn load(name: &str) -> Result<Prompt> {
        // Either we make sure included templates are rendered as static, or swiftide Templates
        // should be Cow
        let byte_file =
            Templates::get(name).with_context(|| format!("Expected template {name}"))?;
        let template = std::str::from_utf8(&byte_file.data)?.to_string();

        Ok(template.into())
    }
}
