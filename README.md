<details>
  <summary>Table of Contents</summary>

<!--toc:start-->

- [What is Kwaak?](#what-is-kwaak)
- [High level features](#high-level-features)
- [Latest updates on our blog :fire:](#latest-updates-on-our-blog-fire)
- [Getting started](#getting-started)
  - [Requirements](#requirements)
  - [Installation and setup](#installation-and-setup)
    - [Homebrew](#homebrew)
    - [Linux and MacOS (using curl)](#linux-and-macos-using-curl)
    - [Cargo](#cargo)
    - [Setup](#setup)
  - [Running Kwaak](#running-kwaak)
  - [How does it work?](#how-does-it-work)
  - [Configuration](#configuration)
    - [General Configuration](#general-configuration)
    - [Command Configuration](#command-configuration)
    - [API Key Management](#api-key-management)
    - [Docker and GitHub Configuration](#docker-and-github-configuration)
    - [LLM Configuration](#llm-configuration)
    - [Other integrations](#other-integrations)
- [Upcoming](#upcoming)
- [Troubleshooting & FAQ](#troubleshooting-faq)
- [Community](#community)
- [Contributing](#contributing)
- [License](#license)
<!--toc:end-->

</details>

<a name="readme-top"></a>

<!-- PROJECT SHIELDS -->
<!--
*** I'm using markdown "reference style" links for readability.
*** Reference links are enclosed in brackets [ ] instead of parentheses ( ).
*** See the bottom of this document for the declaration of the reference variables
*** for contributors-url, forks-url, etc. This is an optional, concise syntax you may use.
*** https://www.markdownguide.org/basic-syntax/#reference-style-links
-->

![CI](https://img.shields.io/github/actions/workflow/status/bosun-ai/kwaak/tests.yml?style=flat-square)
![Coverage Status](https://img.shields.io/coverallsCoverage/github/bosun-ai/kwaak?style=flat-square)
[![Crate Badge]][Crate]
[![Contributors][contributors-shield]][contributors-url]
[![Stargazers][stars-shield]][stars-url]
[![MIT License][license-shield]][license-url]
[![LinkedIn][linkedin-shield]][linkedin-url]
![Discord](https://img.shields.io/discord/1257672801553354802?style=flat-square&link=https%3A%2F%2Fdiscord.gg%2F3jjXYen9UY)

<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://github.com/bosun-ai/kwaak">
    <img src="https://github.com/bosun-ai/kwaak/blob/master/images/logo.webp" alt="Logo" width="250" height="250">
  </a>

  <h3 align="center">Kwaak</h3>

  <p align="center">
Burn through tech debt with AI agents!<br />
    <br />
    <br />
    <!-- <a href="https://github.com/bosun-ai/swiftide">View Demo</a> -->
    <a href="https://github.com/bosun-ai/kwaak/issues/new?labels=bug&template=bug_report.md">Report Bug</a>
    ·
    <a href="https://github.com/bosun-ai/kwaak/issues/new?labels=enhancement&template=feature_request.md">Request Feature</a>
    ·
    <a href="https://discord.gg/3jjXYen9UY">Discord</a>
  </p>
</div>

 <!-- ABOUT THE PROJECT -->

## What is Kwaak?

 <!-- [![Product Name Screen Shot][product-screenshot]](https://example.com) -->

Always wanted to run a team of AI agents locally from your own machine? Write code, improve test coverage, update documentation, or improve code quality, while you focus on building the cool stuff? Kwaak enables you to run a team of autonomous AI agents right from your terminal, **in parallel**.

Kwaak is free and open-source. You can bring your own API keys, or your own models via Ollama.

<p align="center">

![demo](./images/demo.gif)

</p>

Kwaak is aware of your codebase and can answer questions about your code, find examples, write and execute code, create pull requests, and more. Unlike other tools, Kwaak is focussed on autonomous agents, and can run multiple agents at the same time.

> [!CAUTION]
> Kwaak can be considered alpha software. The project is under active development; expect breaking changes. Contributions, feedback, and bug reports are very welcome.

Kwaak is part of the [bosun.ai](https://bosun.ai) project. An upcoming platform for autonomous code improvement.

Powered by [Swiftide](https://github.com/bosun-ai/swiftide)

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## High level features

- Run multiple agents in parallel
- Quacking terminal interface
- As fast as it gets; written in Rust, powered by Swiftide
- Agents operate on code, use tools, and can be interacted with
- View and pull code changes from an agent; or have it create a pull request
- Sandboxed execution in docker
- OpenAI, Ollama, Anthropic, and many other models via [OpenRouter](https://openrouter.ai)
- Python, TypeScript/Javascript, Go, Java, Ruby, Solidity, and Rust

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Latest updates on our blog :fire:

- [Releasing kwaak with kwaak](https://bosun.ai/posts/releasing-kwaak-with-kwaak/)

## Getting started

### Requirements

Before you can run Kwaak, make sure you have Docker installed on your machine.

#### Docker

Kwaak expects a Dockerfile in the root of your project. If you already have a Dockerfile, you can just name it differently and configure it in the configuration file. This Dockerfile should contain all the dependencies required to test and run your code.

> [!NOTE]
> Docker is used to provide a safe execution environment for the agents. It does not affect the performance of the LLMs. The LLMs are running either locally or in the cloud, and the docker container is only used to run the code. This is done to ensure that the agents cannot access your local system. Kwaak itself runs locally.

Additionally, the Dockerfile expects `git` and should be `ubuntu` based.

A simple example for Rust:

```Dockerfile
FROM rust:latest

RUN apt-get update && apt install git -y --no-install-recommends

COPY . /app

WORKDIR /app
```

If you already have a Dockerfile for other purposes, you can either extend it or provide a new one and override the dockerfile path in the configuration.

_For an example Dockerfile in Rust, see [this project's Dockerfile](/Dockerfile)_

#### Api keys

Additionally, you will need an API key for your LLM of choice.

If you'd like kwaak to be able to make pull requests, search github code, and automatically push to a remote, a [github token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens).

<p align="right">(<a href="#readme-top">back to top</a>)</p>

### Installation and setup

Pre-built binaries are available from the [releases page](https://github.com/bosun-ai/kwaak/releases).

#### Homebrew

```shell
brew install bosun-ai/tap/kwaak
```

#### Linux and MacOS (using curl)

```shell
 curl --proto '=https' --tlsv1.2 -LsSf https://github.com/bosun-ai/kwaak/releases/latest/download/kwaak-installer.sh | sh
```

#### Cargo

```shell
cargo install kwaak
```

#### Setup

Once installed, you can run `kwaak init` in the project you want to use Kwaak in. This will guide you through the setup process and it will help you create a configuration file. See [Configuration](#configuration) for more customization options.

Api keys can be prefixed by `env:`, `text:` and `file:` to read secrets from the environment, a text string, or a file respectively.

We highly recommend taking a look at the configuration file and adjusting it to your needs. There are various options that can improve the performance and accuracy of the agents.

### Running Kwaak

You can then run `kwaak` in the root of your project. On initial bootup, Kwaak will index your codebase. This can take a while, depending on the size. Once indexing has been completed, subsequent startups will be faster.

Keybindings:

- **_ctrl-s_**: Send the current message to the agent
- **_ctrl-x_**: Exit the agent
- **_ctrl-q_**: Exit kwaak
- **_ctrl-n_**: Create a new agent
- **_Page Up_**: Scroll chat up
- **_Page Down_**: Scroll chat down
- **_tab_**: Switch between agents

Additionally, kwaak provides a number of slash commands, `/help` will show all available commands.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

### How does it work?

On initial boot up, Kwaak will index your codebase. This can take a while, depending on the size. Once indexing has been completed once, subsequent startups will be faster. Indexes are stored with [lancedb](https://github.com/lancedb/lancedb), and indexing is cached with [redb](https://github.com/cberner/redb).

Kwaak provides a chat interface similar to other LLM chat applications. You can type messages to the agent, and the agent will try to accomplish the task and respond.

When starting a chat, the code of the current branch is copied into an on-the-fly created docker container. This container is then used to run the code and execute the commands.

After each chat completion, kwaak will lint, commit, and push the code to the remote repository if any code changes have been made. Kwaak can also create a pull request. Pull requests include an issue link to #48. This helps us identify the success rate of the agents, and also enforces transparency for code reviewers.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

### Configuration

Kwaak supports configuring different Large Language Models (LLMs) for distinct tasks like indexing, querying, and embedding to optimize performance and accuracy. Be sure to tailor the configurations to fit the scope and blend of the tasks you're tackling.

#### General Configuration

All of these are inferred from the project directory and can be overridden in the `kwaak.toml` configuration file.

- **`project_name`**: Defaults to the current directory name. Represents the name of your project.
- **`language`**: The programming language of the project, for instance, Rust, Go, Python, JavaScript, etc.

#### Command Configuration

Kwaak uses tests, coverages, and lints as an additional opportunity to steer the agent. Configuring these will (significantly) improve the agents' performance.

- **`test`**: Command to run tests, e.g., `cargo test`.
- **`coverage`**: Command for running coverage checks, e.g., `cargo llvm-cov --summary-only`. Expects coverage results as output. Currently handled unparsed via an LLM call. A friendly output is preferred
- **`lint_and_fix`**: Optional command to lint and fix project issues, e.g., `cargo clippy --fix --allow-dirty; cargo fmt` in Rust.

#### API Key Management

- API keys and tokens can be configured through environment variables (`env:KEY`), directly in the configuration (`text:KEY`), or through files (`file:/path/to/key`).

#### Docker and GitHub Configuration

- **`docker.dockerfile`, `docker.context`**: Paths to Dockerfile and context, default to project root and `Dockerfile`.
- **`github.repository`, `github.main_branch`, `github.owner`, `github.token`**: GitHub repository details and token configuration.

#### LLM Configuration

Supported providers:

- OpenAI
- Ollama
- Anthropic
- OpenRouter (no embeddings)
- FastEmbed (embeddings only)

**OpenAI Configuration**:

```toml
[llm.indexing]
api_key = "env:KWAAK_OPENAI_API_KEY"
provider = "OpenAI"
prompt_model = "gpt-4o-mini"

[llm.query]
api_key = "env:KWAAK_OPENAI_API_KEY"
provider = "OpenAI"
prompt_model = "gpt-4o"

[llm.embedding]
api_key = "env:KWAAK_OPENAI_API_KEY"
provider = "OpenAI"
embedding_model = "text-embedding-3-large"
```

**Ollama Configuration**:

```toml
[llm.indexing]
provider = "Ollama"
prompt_model = "llama3.2"

[llm.query]
provider = "Ollama"
prompt_model = "llama3.3"

[llm.embedding]
provider = "Ollama"
embedding_model = { name = "bge-m3", vector_size = 1024 }
```

For both you can provide a `base_url` to use a custom API endpoint. The `api_key` can be set per provider, or globally.

You can mix and match models from different providers for different tasks.

#### Backoff Configuration

Kwaak uses the exponential backoff strategy to handle retries. Currently, only
OpenAI, OpenRouter, and Anthropic calls will make use of the backoff parameters.
You can configure the backoff settings in the `kwaak.toml` file under a
`[backoff]` section. These settings are optional, and default to the following
values:

- `initial_interval_sec`: Defaults to 15 seconds. This sets the initial waiting time between retries.
- `multiplier`: Defaults to 2.0. This factor multiplies the interval on each retry attempt.
- `randomization_factor`: Defaults to 0.05. Introduces randomness to avoid retry storms.
- `max_elapsed_time_sec`: Defaults to 120 seconds. This total time all attempts are allowed.

Example Configuration:

```toml
[backoff]
initial_interval_sec = 15
multiplier = 2.0
randomization_factor = 0.05
max_elapsed_time_sec = 120
```

#### Other configuration

- **`agent_custom_constraints`**: Additional constraints / instructions for the agent.
  These are passes to the agent in the system prompt and are rendered in a list. If you
  intend to use more complicated instructions, consider adding a file to read in the
  repository instead.
- **`cache_dir`, `log_dir`**: Directories for cache and logs. Defaults are within your system's cache directory.
- **`indexing_concurrency`**: Adjust concurrency for indexing, defaults based on CPU count.
- **`indexing_batch_size`**: Batch size setting for indexing. Defaults to a higher value for Ollama and a lower value for OpenAI.
- **`endless_mode`**: **DANGER** If enabled, agents run continuously until manually stopped or completion is reached. This is meant for debugging and evaluation purposes.
- **`otel_enabled`**: Enables OpenTelemetry tracing if set and respects all the standard OpenTelemetry environment variables.
- **`tool_executor`**: Defaults to `docker`. Can also be `local`. We **HIGHLY** recommend using `docker` for security reasons unless you are running in a secure environment.
- **`tavily_api_key`**: Enables the agent to use [tavily](https://tavily.com) for web search. Their entry-level plan is free. (we are not affiliated)
- **`agent_edit_mode`**: Defaults to `whole` (write full files at the time). If you experience issues with (very) large files, you can experiment with `line` edits.
- **`git.auto_push_remote`**: Enabled by default if a github key is present. Automatically pushes to the remote repository after each chat completion. You can disable this by setting it to `false`.
- **`git.auto_commit_disabled`**: Opt-out of automatic commits after each chat completion.
- **`disabled_tools.pull_request`**: Enables or disables the pull request tool. Defaults to `false`.
- **`ui.hide_header`**: Optionally hide the top header in the UI. Defaults to `false`.
- **`num_completions_for_summary`**: Number of completions before the agent summarizes the conversation. Defaults to 10; 
- **`git.agent_user_name`**: Name which the kwaak agent will make commands with.
  Defaults to "kwaak"` 
- **`git.agent_user_email`**: Email which the kwaak agent will make commits
  with. Defaults to "kwaak@bosun.ai"

<!-- ROADMAP -->

## Upcoming

- Support for more LLMs
- Tools for code documentation
- More sources for additional context
- ... and more! (we don't really have a roadmap, but we have a lot of ideas)

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- CONTRIBUTING -->

## Troubleshooting & FAQ

**Q:** I get a lot of failures, errors, or otherwise unexpected behaviour from the agent.
**A:** Make sure you are _not_ on a Tier 1 account with either OpenAI or Anthropic, the token limits are not enough to run coding agents. Additionally, you can also experiment with different edit modes. See `agent_edit_mode`. If you only have Tier 1 accounts, you can also consider using OpenRouter, which does not have these limits. It is generally a bit slower and less reliable.

**Q:** Kwaak feels very slow

**A:** Try increasing the resources available for docker. For docker desktop this is in Settings -> Resources -> Advanced. On MacOS, adding your terminal and/or kwaak to developer tools can also help.

**Q:**: There was an error during a chat, have I lost all progress?

**A:** Kwaak commits and pushes to the remote repository after each completion, so you should be able to recover the changes.

**Q:** I get a redb/lancedb error when starting, what is up?

**A**: Possibly your index got corrupted, or you have another kwaak instance running on the same project. Try clearing the index with `kwaak clear-index` and restart kwaak. Note that this will require a reindexing of your codebase.

**Q:** I get an `error from Bollard: Socket not found /var/run/docker.sock`

**A**: Enable the default Docker socket in docker desktop in General -> Advanced settings.

**Q:** I get an `data did not match any variant of untagged enum LLMConfigurations`

**A**: Make sure any env variables are set correctly, and that the configuration file is correct. This is unfortunately a very generic error.

**Q:** So why docker? How does it work? Does it affect local LLM performance?

**A**: Docker is only used to provide a save execution environment for the agents. It does not affect the performance of the LLMs. The LLMs are running either locally or in the cloud, and the docker container is only used to run the code. This is done to ensure that the agents cannot access your local system. Kwaak itself runs locally.

**Q**: What is the github token used for?

**A**: The github token is used to create pull requests, search code, and push code to a remote repository. It is not used for anything else.

**Q**: In my project, different contributors have different setups. How can I make sure kwaak works for everyone?

**A**: You can use a `kwaak.local.toml` and add it to your `.gitignore`. Alternatively, all configuration can be overridden by environment variables, prefixed with `KWAAK` and separated by double underscores. For instance, `KWAAK__COMMAND_TEST=cargo nextest run`. Overwriting via environment currently does not work for the `llm` configuration.

## Community

If you want to get more involved with `kwaak`, have questions or want to chat, you can find us on [discord](https://discord.gg/3jjXYen9UY).

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Contributing

If you have a great idea, please fork the repo and create a pull request.

Don't forget to give the project a star! Thanks again!

Testing agents is not a trivial matter. We have benchmarks to verify agent behaviour across larger datasets. Check out the [Evaluations](EVALUATIONS.md) page for more information as well as the [SWE-Bench](benchmarks/swe/README.md) benchmark page for more information.

If you just want to contribute (bless you!), see [our issues](https://github.com/bosun-ai/kwaak/issues) or join us on Discord.

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'feat: Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

See [CONTRIBUTING](https://github.com/bosun-ai/swiftide/blob/master/CONTRIBUTING.md) for more

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- LICENSE -->

## License

Distributed under the MIT License. See `LICENSE` for more information.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- MARKDOWN LINKS & IMAGES -->
<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->

[contributors-shield]: https://img.shields.io/github/contributors/bosun-ai/kwaak.svg?style=flat-square
[contributors-url]: https://github.com/bosun-ai/kwaak/graphs/contributors
[stars-shield]: https://img.shields.io/github/stars/bosun-ai/kwaak.svg?style=flat-square
[stars-url]: https://github.com/bosun-ai/kwaak/stargazers
[license-shield]: https://img.shields.io/github/license/bosun-ai/kwaak.svg?style=flat-square
[license-url]: https://github.com/bosun-ai/kwaak/blob/master/LICENSE.txt
[linkedin-shield]: https://img.shields.io/badge/-LinkedIn-black.svg?style=flat-square&logo=linkedin&colorB=555
[linkedin-url]: https://www.linkedin.com/company/bosun-ai
[Crate Badge]: https://img.shields.io/crates/v/kwaak?logo=rust&style=flat-square&logoColor=E05D44&color=E05D44
[Crate]: https://crates.io/crates/kwaak
[Docs Badge]: https://img.shields.io/docsrs/kwaak?logo=rust&style=flat-square&logoColor=E05D44
[API Docs]: https://docs.rs/kwaak
