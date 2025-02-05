use anyhow::Result;
use strum::IntoEnumIterator;
use swiftide::integrations::treesitter::SupportedLanguages;

use crate::config::defaults::default_project_name;

use super::util::{prompt_select, prompt_text};

pub fn project_questions(context: &mut tera::Context) -> Result<()> {
    let project_name = default_project_name();
    let project_name_input = prompt_text("Project name", Some(&project_name)).prompt()?;
    context.insert("project_name", &project_name_input);

    // Get user inputs with defaults
    let detected = naive_lang_detect();
    let options = SupportedLanguages::iter()
        .map(|l| l.to_string())
        .collect::<Vec<_>>();

    let language_input = prompt_select("Programming language", options.clone(), detected)?;

    context.insert("language", &language_input);

    Ok(())
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
