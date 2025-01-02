use anyhow::Context as _;
use anyhow::Result;
use rust_embed::Embed;
use tera::Tera;

#[derive(Embed)]
#[folder = "templates"]
pub struct Templates;

// TODO: Would be nice if we could combine/replace this in a nice way with swiftide templates
impl Templates {
    pub fn render(name: &str, context: &tera::Context) -> Result<String> {
        let byte_file =
            Templates::get(name).with_context(|| format!("Expected template {name}"))?;
        let template = std::str::from_utf8(&byte_file.data)?;

        Tera::one_off(template, context, false).context("Failed to render template")
    }

    // Should be zero need to allocate here but lazy day
    pub fn from_file(name: &str) -> Result<String> {
        let byte_file =
            Templates::get(name).with_context(|| format!("Expected template {name}"))?;

        String::from_utf8(byte_file.data.into_owned()).context("Failed to read template")
    }
}
