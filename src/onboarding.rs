use crate::{
    config::{
        defaults::{default_main_branch, default_owner_and_repo, default_project_name},
        LLMConfiguration, OpenAIEmbeddingModel, OpenAIPromptModel,
    },
    templates::Templates,
};
use anyhow::{Context as _, Result};
use serde_json::json;
use strum::{IntoEnumIterator as _, VariantNames};
use swiftide::integrations::treesitter::SupportedLanguages;

pub fn run(dry_run: bool) -> Result<()> {
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
    println!("We have a few questions to ask you to get started, you can always change these later in the `kwaak.toml` file.");

    let mut context = tera::Context::new();
    project_questions(&mut context);
    git_questions(&mut context);
    llm_questions(&mut context);

    let config =
        Templates::render("kwaak.toml", &context).context("Failed to render default config")?;

    debug_assert!(
        toml::from_str::<crate::config::Config>(&config).is_ok(),
        "Failed to parse the rendered config with error: {error}, config: \n{config}",
        error = toml::from_str::<crate::config::Config>(&config).unwrap_err()
    );

    // Since we want the template annotated with comments, just return the template
    if dry_run {
        println!("Dry run, would have written the following to kwaak.toml:\n\n{config}");
    } else {
        std::fs::write("kwaak.toml", &config)?;
        println!("Initialized kwaak project in current directory, please review and customize the created `kwaak.toml` file.\n Kwaak also needs a `Dockerfile` to execute your code in, with `ripgrep` and `fd` installed. Refer to https://github.com/bosun-ai/kwaak for an up to date list.");
    }

    Ok(())
}

fn prompt_text<'a>(prompt: &'a str, default: Option<&'a str>) -> inquire::Text<'a> {
    let mut prompt = inquire::Text::new(prompt);

    if let Some(default) = default {
        prompt = prompt.with_default(default);
    }

    prompt
}

fn prompt_api_key<'a>(prompt: &'a str, default: Option<&'a str>) -> inquire::Text<'a> {
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
fn prompt_select<T>(prompt: &str, options: Vec<T>, default: Option<T>) -> String
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

    prompt.prompt().unwrap().to_string()
}

fn project_questions(context: &mut tera::Context) {
    let project_name = default_project_name();
    let project_name_input = prompt_text("Project name", Some(&project_name))
        .prompt()
        .unwrap();
    context.insert("project_name", &project_name_input);

    // Get user inputs with defaults
    let detected = naive_lang_detect();
    let options = SupportedLanguages::iter()
        .map(|l| l.to_string())
        .collect::<Vec<_>>();

    let language_input = prompt_select("Programming language", options.clone(), detected);

    context.insert("language", &language_input);
}

fn git_questions(context: &mut tera::Context) {
    let (default_owner, default_repository) = default_owner_and_repo().unzip();
    let default_branch = default_main_branch();
    let branch_input = prompt_text("Default git branch", Some(&default_branch))
        .prompt()
        .unwrap();

    println!("With a github token, Kwaak can create pull requests, search github code, and automatically push to the remote.");
    let github_api_key = prompt_api_key(
        "GitHub api key (optional, <esc> to skip)",
        Some("env:GITHUB_TOKEN"),
    )
    .prompt_skippable()
    .unwrap();

    let auto_push_remote =
        inquire::Confirm::new("Push to git remote after changes? (requires github token)")
            .with_default(github_api_key.is_some())
            .prompt()
            .unwrap();

    let owner_input = prompt_text(
        "Git owner (optional, <esc> to skip)",
        default_owner.as_deref(),
    )
    .prompt_skippable()
    .unwrap();
    let repository_input = prompt_text(
        "Git repository (optional, <esc> to skip)",
        default_repository.as_deref(),
    )
    .prompt_skippable()
    .unwrap();

    context.insert("github_api_key", &github_api_key);
    context.insert(
        "git",
        &json!({
            "owner": owner_input,
            "repository": repository_input,
            "main_branch": branch_input,
            "auto_push_remote": auto_push_remote,

        }),
    );
}

fn llm_questions(context: &mut tera::Context) {
    let valid_llms = LLMConfiguration::VARIANTS;

    let valid_llm: LLMConfiguration = prompt_select(
        "What LLM would you like to use?",
        valid_llms.to_vec(),
        Some("OpenAI"),
    )
    .parse()
    .unwrap();

    match valid_llm {
        LLMConfiguration::OpenAI { .. } => openai_questions(context),
        LLMConfiguration::Ollama { .. } => ollama_questions(context),
        #[cfg(debug_assertions)]
        LLMConfiguration::Testing => {
            println!("{valid_llm} is not meant for production use, skipping configuration");
        }
    }
}

fn openai_questions(context: &mut tera::Context) {
    let api_key = prompt_api_key(
        "Where can we find your OpenAI api key? (https://platform.openai.com/api-keys)",
        Some("env:OPENAI_API_KEY"),
    )
    .prompt()
    .unwrap();
    let indexing_model = prompt_select(
        "Model used for fast operations (like indexing)",
        OpenAIPromptModel::VARIANTS.to_vec(),
        Some("gpt-4o-mini"),
    );
    let query_model = prompt_select(
        "Model used for querying and code generation",
        OpenAIPromptModel::VARIANTS.to_vec(),
        Some("gpt-4o"),
    );

    let embedding_model = prompt_select(
        "Model used for embeddings",
        OpenAIEmbeddingModel::VARIANTS.to_vec(),
        Some("text-embedding-3-large"),
    );

    let base_url = inquire::Text::new("Custom base url (optional, <esc> to skip)")
        .with_validator(|input: &str| match url::Url::parse(input) {
            Ok(_) => Ok(inquire::validator::Validation::Valid),
            Err(_) => Ok(inquire::validator::Validation::Invalid(
                "Invalid URL".into(),
            )),
        })
        .prompt_skippable()
        .unwrap();

    context.insert("openai_api_key", &api_key);
    context.insert(
        "llm",
        &json!({
            "provider": "OpenAI",
            "indexing_model": indexing_model,
            "query_model": query_model,
            "embedding_model": embedding_model,
            "base_url": base_url,
        }),
    );
}

fn ollama_questions(context: &mut tera::Context) {
    println!("Note that you need to have a running Ollama instance.");

    let indexing_model = prompt_text(
        "Model used for fast operations (like indexing). This model does not need to support tool calls.",
        None

    ).prompt().unwrap();

    let query_model = prompt_text(
        "Model used for querying and code generation. This model needs to support tool calls.",
        None,
    )
    .prompt()
    .unwrap();

    let embedding_model = prompt_text("Model used for embeddings, bge-m3 is a solid choice", None)
        .prompt()
        .unwrap();

    let vector_size = inquire::Text::new("Vector size for the embedding model")
        .with_validator(|input: &str| match input.parse::<usize>() {
            Ok(_) => Ok(inquire::validator::Validation::Valid),
            Err(_) => Ok(inquire::validator::Validation::Invalid(
                "Invalid number".into(),
            )),
        })
        .prompt()
        .unwrap();

    let base_url = inquire::Text::new("Custom base url? (optional, <esc> to skip)")
        .with_validator(|input: &str| match url::Url::parse(input) {
            Ok(_) => Ok(inquire::validator::Validation::Valid),
            Err(_) => Ok(inquire::validator::Validation::Invalid(
                "Invalid URL".into(),
            )),
        })
        .prompt_skippable()
        .unwrap();

    context.insert(
        "llm",
        &json!({
            "provider": "Ollama",
            "indexing_model": indexing_model,
            "query_model": query_model,
            "embedding_model": embedding_model,
            "vector_size": vector_size,
            "base_url": base_url,
        }),
    );
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
