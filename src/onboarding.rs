use crate::{
    config::defaults::{default_main_branch, default_owner_and_repo, default_project_name},
    templates::Templates,
};
use anyhow::{Context as _, Result};
use serde_json::json;

pub fn run() -> Result<()> {
    if std::fs::metadata(".git").is_err() {
        anyhow::bail!("Not a git repository, please run `git init` first");
    }
    if std::fs::metadata("kwaak.toml").is_ok() {
        anyhow::bail!("kwaak.toml already exists in current directory, skipping initialization");
    }
    let config = create_template_config()?;
    std::fs::write("kwaak.toml", &config)?;

    println!("Initialized kwaak project in current directory, please review and customize the created `kwaak.toml` file.\n Kwaak also needs a `Dockerfile` to execute your code in, with `ripgrep` and `fd` installed. Refer to https://github.com/bosun-ai/kwaak for an up to date list.");

    Ok(())
}

fn create_template_config() -> Result<String> {
    let mut context = tera::Context::new();

    // Helper for getting user feedback with a default
    fn input_with_default(prompt: &str, default: &str) -> String {
        println!("{prompt} [{default}]: ");
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .expect("Failed to read input");
        let trimmed = input.trim();
        if trimmed.is_empty() {
            default.to_string()
        } else {
            trimmed.to_string()
        }
    }

    // Get user inputs with defaults
    let language = naive_lang_detect().map_or_else(|| "REQUIRED".to_string(), |l| l.to_string());
    let language_input = input_with_default("Enter the programming language", &language);
    context.insert("language", &language_input);

    let project_name = default_project_name();
    let project_name_input = input_with_default("Enter the project name", &project_name);
    context.insert("project_name", &project_name_input);

    let (default_owner, default_repository) = default_owner_and_repo();
    let owner_input = input_with_default("Enter the GitHub owner/org", &default_owner);
    let repository_input = input_with_default("Enter the GitHub repository", &default_repository);
    let default_branch = default_main_branch();
    let branch_input = input_with_default("Enter the main branch", &default_branch);

    context.insert(
        "github",
        &json!({
            "owner": owner_input,
            "repository": repository_input,
            "main_branch": branch_input,

        }),
    );

    let config =
        Templates::render("kwaak.toml", &context).context("Failed to render default config")?;

    // Since we want the template annotated with comments, just return the template
    Ok(config)
}
fn naive_lang_detect() -> Option<String> {
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
        ("go.mod", "Go"),
    ];

    // Iterate through the files and detect the language
    for (file, language) in &language_files {
        if std::fs::metadata(file).is_ok() {
            return Some((*language).to_string());
        }
    }

    None
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_valid_template() {
        // Clean up env variables for a pure test
        std::env::vars().for_each(|(key, _)| {
            if key.starts_with("KWAAK") {
                std::env::remove_var(key);
            }
        });
        std::env::set_var("KWAAK_OPENAI_API_KEY", "test");
        std::env::set_var("KWAAK_GITHUB_TOKEN", "test");
        let config = create_template_config().unwrap();

        toml::from_str::<crate::config::Config>(&config).unwrap();
    }
}
