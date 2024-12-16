use crate::{
    config::defaults::{default_main_branch, default_owner_and_repo, default_project_name},
    templates::Templates,
};
use anyhow::{Context as _, Result};
use serde_json::json;
use swiftide::integrations::treesitter::SupportedLanguages;

pub fn create_template_config() -> Result<String> {
    let mut context = tera::Context::new();

    context.insert(
        "language",
        &naive_lang_detect().map_or_else(|| "REQUIRED".to_string(), |l| l.to_string()),
    );
    context.insert("project_name", &default_project_name());

    let (owner, repository) = default_owner_and_repo();
    context.insert(
        "github",
        &json!({
            "owner": owner,
            "repository": repository,
            "main_branch": default_main_branch(),

        }),
    );

    let config =
        Templates::render("kwaak.toml", &context).context("Failed to render default config")?;

    // Since we want the template annotated with comments, just return the template
    Ok(config)
}

fn naive_lang_detect() -> Option<SupportedLanguages> {
    // Check for major package manager files to detect the language
    // Then return the first language found
    let language_files = [
        ("Cargo.toml", "Rust"),
        ("Gemfile", "Ruby"),
        ("tsconfig.json", "Typescript"),
        ("package.json", "Javascript"),
        ("pyproject.toml", "Python"),
        ("requirements.txt", "Python"),
        ("Pipfile", "Python"),
        ("build.gradle", "Java"),
        ("pom.xml", "Java"),
    ];

    // Iterate through the files and detect the language
    for (file, language) in language_files {
        if std::fs::metadata(file).is_ok() {
            return language.parse().ok();
        }
    }

    None
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_valid_template() {
        let config = create_template_config().unwrap();
        dbg!(&config);

        toml::from_str::<crate::config::Config>(&config).unwrap();
    }
}
