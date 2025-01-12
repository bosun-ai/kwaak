<details>
  <summary>Table of Contents</summary>

<!--toc:start-->

- [What is Kwaak?](#what-is-kwaak)
- [Features](#features)
- [Getting started](#getting-started)
  - [Requirements](#requirements)
  - [Installation and setup](#installation-and-setup)
  - [Running Kwaak](#running-kwaak)
  - [Configuration](#configuration)
- [Example prompts](#example-prompts)
- [Roadmap](#roadmap)
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
    Run a team of autonomous AI agents on your code, right from your terminal!
    <br />
    <a href="https://swiftide.rs"><strong>Powered by swiftide »</strong></a>
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

<p align="center">

![demo](./images/demo.gif)

</p>

Powered by [Swiftide](https://github.com/bosun-ai/swiftide), Kwaak is aware of your codebase and can answer questions about your code, find examples, write and execute code, create pull requests, and more. Unlike other tools, Kwaak is focussed on autonomous agents, and can run multiple agents at the same time.

> [!CAUTION]
> Kwaak can be considered alpha software. The project is under active development; expect breaking changes. Contributions, feedback, and bug reports are very welcome.

Kwaak is part of the [bosun.ai](https://bosun.ai) project. An upcoming platform for autonomous code improvement.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## High level features

- Run multiple agents in parallel
- Quacking terminal interface
- As fast as it gets; written in Rust, powered by Swiftide
- Agents operate on code, use tools, and can be interacted with
- Sandboxed execution in docker
- Python, TypeScript/Javascript, Java, Ruby, and Rust

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Latest updates on our blog :fire:

- [Releasing kwaak with kwaak](https://bosun.ai/posts/releasing-kwaak-with-kwaak/)

## Getting started

### Requirements

Before you can run Kwaak, make sure you have Docker installed on your machine.

Kwaak expects a Dockerfile in the root of your project. This Dockerfile should contain all the dependencies required to test and run your code. Additionally, it expects the following to be present:

- **git**: Required for git operations
- **fd** [github](https://github.com/sharkdp/fd): Required for searching files. Note that it should be available as `fd`, some systems have it as `fdfind`.
- **ripgrep** [github](https://github.com/BurntSushi/ripgrep): Required for searching _in_ files. Note that it should be available as `rg`.

If you already have a Dockerfile for other purposes, you can either extend it or provide a new one and override the dockerfile path in the configuration.

_For an example Dockerfile in Rust, see [this project's Dockerfile](/Dockerfile)_

Additionally, you will need an OpenAI API key (if OpenAI is your LLM provider) and a [github token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens).

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

Once installed, you can run `kwaak init` in the project you want to use Kwaak in. This will create a `kwaak.toml` in your project root. You can edit this file to configure Kwaak.

After verifying the default configuration, one required step is to set up the `test` and `coverage` commands. There are also some optional settings you can consider.

Api keys can be prefixed by `env:`, `text:` and `file:` to read secrets from the environment, a text string, or a file respectively.

### Running Kwaak

You can then run `kwaak` in the root of your project. This will start the Kwaak terminal interface. On initial bootup, Kwaak will index your codebase. This can take a while, depending on the size. Once indexing has been completed, subsequent startups will be faster.

Keybindings:

- **_ctrl-s_**: Send the current message to the agent
- **_ctrl-x_**: Exit the agent
- **_ctrl-q_**: Exit kwaak
- **_ctrl-n_**: Create a new agent
- **_Page Up_**: Scroll chat up
- **_Page Down_**: Scroll chat down
- **_tab_**: Switch between agents

<p align="right">(<a href="#readme-top">back to top</a>)</p>

### Configuration

Kwaak supports configuring different Large Language Models (LLMs) for distinct tasks like indexing, querying, and embedding to optimize performance and accuracy. Be sure to tailor the configurations to fit the scope and blend of the tasks you're tackling.

#### OpenAI Configuration:

Edit the `kwaak.toml` file to add your OpenAI settings for different tasks:

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

#### Ollama Configuration:

You can set up similar configurations for Ollama. Make sure the model names and options align with your requirements:

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

These configurations allow you to leverage the strengths of each model effectively for indexing, querying, and embedding processes.

### How does it work?

On initial boot up, Kwaak will index your codebase. This can take a while, depending on the size. Once indexing has been completed once, subsequent startups will be faster. Indexes are stored with [lancedb](https://github.com/lancedb/lancedb), and indexing is cached with [redb](https://github.com/cberner/redb).

Kwaak provides a chat interface similar to other LLM chat applications. You can type messages to the agent, and the agent will try to accomplish the task and respond.

When starting a chat, the code of the current branch is copied into an on-the-fly created docker container. This container is then used to run the code and execute the commands.

After each chat completion, kwaak will lint, commit, and push the code to the remote repository if any code changes have been made. Kwaak can also create a pull request. Pull requests include an issue link to #48. This helps us identify the success rate of the agents, and also enforces transparency for code reviewers.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- ROADMAP -->

## Upcoming

- Support for more LLMs
- Tools for code documentation
- More sources for additional context
- ... and more! (we don't really have a roadmap, but we have a lot of ideas)

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- CONTRIBUTING -->

## Troubleshooting & FAQ

**Q:** Kwaak feels very slow

**A:** Try increasing the resources available for docker. For docker desktop this is in Settings -> Resources -> Advanced. On MacOS, adding your terminal and/or kwaak to developer tools can also help.

**Q:**: There was an error during a chat, have I lost all progress?

**A:** Kwaak commits and pushes to the remote repository after each completion, so you should be able to recover the changes.

**Q:** I get a redb/lancedb error when starting, what is up?

**A**: Possibly your index got corrupted, or you have another kwaak instance running on the same project. Try clearing the index with `kwaak clear-index` and restart kwaak. Note that this will require a reindexing of your codebase.

**Q:** I get an `error from Bollard: Socket not found /var/run/docker.sock`

**A**: Enable the default Docker socket in docker desktop in General -> Advanced settings.

## Community

If you want to get more involved with `kwaak`, have questions or want to chat, you can find us on [discord](https://discord.gg/3jjXYen9UY).

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Contributing

If you have a great idea, please fork the repo and create a pull request.

Don't forget to give the project a star! Thanks again!

Testing agents is not a trivial matter. We have (for now) internal benchmarks to verify agent behaviour across larger datasets.

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
