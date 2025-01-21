use std::{io::Write as _, str::FromStr as _};

use crate::{
    config::{
        defaults::{default_main_branch, default_owner_and_repo, default_project_name},
        LLMConfiguration,
    },
    templates::Templates,
};
use anyhow::{Context as _, Result};
use serde_json::json;
use strum::{IntoEnumIterator as _, VariantNames};
use swiftide::integrations::treesitter::SupportedLanguages;

pub fn run(dry_run: bool) -> Result<()> {
    // Onboarding steps
    // 1. Questions for general setup
    // 2. Configure the llm
    // 3. Ask for other api keys
    // 3. Generate a dockerfile if possible

    if !dry_run {
        if std::fs::metadata(".git").is_err() {
            anyhow::bail!("Not a git repository, please run `git init` first");
        }
        if std::fs::metadata("kwaak.toml").is_ok() {
            anyhow::bail!(
                "kwaak.toml already exists in current directory, skipping initialization"
            );
        }
    }

    println!("Welcome to Kwaak! Let's get started by initializing a new configuration file.");
    println!("\n");

    let mut context = tera::Context::new();
    project_questions(&mut context);
    git_questions(&mut context);
    llm_questions(&mut context);

    let config =
        Templates::render("kwaak.toml", &context).context("Failed to render default config")?;

    // Since we want the template annotated with comments, just return the template
    if dry_run {
        println!("Dry run, would have written the following to kwaak.toml:\n\n{config}");
    } else {
        std::fs::write("kwaak.toml", &config)?;
        println!("Initialized kwaak project in current directory, please review and customize the created `kwaak.toml` file.\n Kwaak also needs a `Dockerfile` to execute your code in, with `ripgrep` and `fd` installed. Refer to https://github.com/bosun-ai/kwaak for an up to date list.");
    }

    Ok(())
}

// Helper for getting user feedback with a default
fn input_with_default(prompt: &str, default: &str) -> String {
    print!("{prompt} [{default}]: ");
    std::io::stdout().flush().unwrap();
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

fn project_questions(context: &mut tera::Context) {
    let project_name = default_project_name();
    let project_name_input = input_with_default("Enter the project name", &project_name);
    context.insert("project_name", &project_name_input);

    // Get user inputs with defaults
    let language = naive_lang_detect().map_or_else(|| "REQUIRED".to_string(), |l| l.to_string());
    let language_input = input_with_default(
        &format!(
            "Enter the programming language ({})",
            SupportedLanguages::iter()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        &language,
    );
    context.insert("language", &language_input);
}

fn git_questions(context: &mut tera::Context) {
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
}

fn llm_questions(context: &mut tera::Context) {
    let valid_llms = LLMConfiguration::VARIANTS;

    let mut valid_llm = None;

    while valid_llm.is_none() {
        let llm_input = input_with_default(
            &format!(
                "What LLM would you like to use? ({})",
                valid_llms.join(", ")
            ),
            "OpenAI",
        );
        let Ok(llm) = LLMConfiguration::from_str(&llm_input) else {
            println!("Invalid LLM, please try again");
            continue;
        };
        valid_llm = Some(llm);
    }

    let valid_llm = valid_llm.unwrap();
    match valid_llm {
        LLMConfiguration::OpenAI { .. } => openai_questions(context),
        LLMConfiguration::Ollama { .. } => ollama_questions(context),
        _ => println!("{} currently should be configured manually", valid_llm),
    }

    // Handle the OpenAI specific questions
    // Handle Ollama specific questions
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

// #[cfg(test)]
// mod test {
//     use super::*;
//
//     #[test]
//     fn test_valid_template() {
//         // Clean up env variables for a pure test
//         std::env::vars().for_each(|(key, _)| {
//             if key.starts_with("KWAAK") {
//                 std::env::remove_var(key);
//             }
//         });
//         std::env::set_var("KWAAK_OPENAI_API_KEY", "test");
//         std::env::set_var("KWAAK_GITHUB_TOKEN", "test");
//         let config = create_template_config().unwrap();
//
//         toml::from_str::<crate::config::Config>(&config).unwrap();
//     }
// }
