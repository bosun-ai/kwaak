//! This modules provides the onboarding flow for Kwaak.
//!
//! It asks the user a series of questions to generate a basic configuration file.
//!
//! Intention is to provide a smooth onboarding experience, not to support every possible
//! configuration.
//!
//! Currently all values are inserted into a tera context, which is then rendered into the
//! `kwaak.toml` template.
//!
//! In the future it would be much nicer if it builds an actual `Config` struct. Then this can also
//! be used for
use std::path::PathBuf;

use crate::templates::Templates;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use commands::command_questions;
use git::git_questions;
use llm::llm_questions;
use project::project_questions;

mod commands;
mod git;
mod llm;
mod project;
mod util;

pub async fn run(file: Option<PathBuf>, dry_run: bool) -> Result<()> {
    let file = file.unwrap_or_else(|| PathBuf::from("kwaak.toml"));
    if !dry_run {
        if std::fs::metadata(".git").is_err() {
            anyhow::bail!("Not a git repository, please run `git init` first");
        }
        if std::fs::metadata(&file).is_ok() {
            anyhow::bail!(
                "{} already exists in current directory, skipping initialization",
                file.display()
            );
        }
    }

    println!("Welcome to Kwaak! Let's get started by initializing a new configuration file.");
    println!("\n");
    println!(
        "We have a few questions to ask you to get started, you can always change these later in the `{}` file.",
        file.display()
    );

    let mut context = tera::Context::new();
    project_questions(&mut context)?;
    git_questions(&mut context)?;
    llm_questions(&mut context).await?;
    command_questions(&mut context)?;

    let config =
        Templates::render("kwaak.toml", &context).context("Failed to render default config")?;

    // Ensure we panic during tests
    debug_assert!(
        toml::from_str::<crate::config::Config>(&config).is_ok(),
        "Failed to parse the rendered config with error: {error:#}, config: \n{config}",
        error = toml::from_str::<crate::config::Config>(&config).unwrap_err()
    );
    toml::from_str::<crate::config::Config>(&config)
        .context("There is an error in the configuration")?;

    // Since we want the template annotated with comments, just return the template
    if dry_run {
        println!("\nDry run, would have written the following to kwaak.toml:\n\n{config}");
    } else {
        std::fs::write(&file, &config)?;
        println!(
            "\nInitialized kwaak project in current directory, please review and customize the created `{}` file.\n Kwaak also needs a `Dockerfile` to execute your code in.",
            file.display()
        );
    }

    // Prompt user for Dockerfile generation after setup
    // Implement calling `generate_dockerfile` after other onboarding steps

    Ok(())
}

pub fn generate_dockerfile(language: &str, output: Option<PathBuf>) -> Result<()> {
    let dockerfile_content = match language {
        "Python" => {
            """
            FROM python:latest

            RUN apt-get update && apt install git -y --no-install-recommends

            
            COPY . /app
            
            WORKDIR /app
            """
        },
        "TypeScript" | "Javascript" => {
            """
            FROM node:latest

            RUN apt-get update && apt install git -y --no-install-recommends

            
            COPY . /app
            
            WORKDIR /app
            """
        },
        "Go" => {
            """
            FROM golang:latest

            RUN apt-get update && apt install git -y --no-install-recommends

            
            COPY . /app
            
            WORKDIR /app
            """
        },
        "Java" => {
            """
            FROM openjdk:latest

            RUN apt-get update && apt install git -y --no-install-recommends

            
            COPY . /app
            
            WORKDIR /app
            """
        },
        "Ruby" => {
            """
            FROM ruby:latest

            RUN apt-get update && apt install git -y --no-install-recommends

            
            COPY . /app
            
            WORKDIR /app
            """
        },
        "Solidity" => {
            """
            FROM ethereum/solc:stable

            
            COPY . /app
            
            WORKDIR /app
            """
        },
        "C" | "C++" => {
            """
            FROM gcc:latest

            RUN apt-get update && apt install git -y --no-install-recommends

            
            COPY . /app
            
            WORKDIR /app
            """
        },
        "Rust" => {
            """
            FROM rust:latest

            RUN apt-get update && apt install git -y --no-install-recommends

            
            COPY . /app
            
            WORKDIR /app
            """
        },
        _ => anyhow::bail!("Language not supported for Dockerfile generation")
    };

    let output_path = output.unwrap_or_else(|| PathBuf::from("Dockerfile"));
    let mut file = File::create(&output_path)?;
    file.write_all(dockerfile_content.as_bytes())?;
    println!("Dockerfile created at {}", output_path.display());

    // Implement calling `generate_dockerfile` after other onboarding steps

    Ok(())
}
