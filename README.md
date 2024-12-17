<details>
  <summary>Table of Contents</summary>

<!--toc:start-->

- [About The Project](#about-the-project)
- [Latest updates on our blog :fire:](#latest-updates-on-our-blog-fire)
- [Example](#example)
- [Vision](#vision)
- [Features](#features)
  - [In detail](#in-detail)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Installation](#installation)
- [Usage and concepts](#usage-and-concepts)
- [Roadmap](#roadmap)
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

![CI](https://img.shields.io/github/actions/workflow/status/bosun-ai/kwaak/test.yml?style=flat-square)
![Coverage Status](https://img.shields.io/coverallsCoverage/github/bosun-ai/kwaak?style=flat-square)
[![Crate Badge]][Crate]
[![Docs Badge]][API Docs]
[![Contributors][contributors-shield]][contributors-url]
[![Stargazers][stars-shield]][stars-url]
[![MIT License][license-shield]][license-url]
[![LinkedIn][linkedin-shield]][linkedin-url]

<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://github.com/bosun-ai/kwaak">
    <img src="https://github.com/bosun-ai/kwaak/blob/master/images/logo.webp" alt="Logo" width="250" height="250">
  </a>

  <h3 align="center">Swiftide</h3>

  <p align="center">
    Run autonomous AI agents on your code!
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

Always wanted to run an army of AI agents locally from your own machine? Kwaak provides a terminal interface to operate autonomous AI agents on your codebase.

<<DEMO HERE>>

Powered by Swiftide, Kwaak is aware of your codebase and can answer questions about your code, find examples, write and execute code, create pull requests, and more. Unlike other tools, Kwaak is focussed on atonomous agents, and can run multiple agents at the same time.

> [!CAUTION]
> Kwaak is in early development and can be considered alpha software. The project is under active development, expect breaking changes. Contributions, feedback, and bug reports are very welcome.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Features

- Run multiple agents at the same time, each with their own copy of the code
- Agents have access to your code, can use a variety of tools, and can be interact with the user
- Currently only works with `OpenAI`. More are planned, the project is not tied to a single LLM.
- Tools are safely executed in a docker based sandbox environment
- Broad language support: Python, TypeScript/Javascript, Java, Ruby, and Rust. Language support is easily extendable, and limited to what Swiftide supports.
- Rich terminal markdown rendering

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Getting started

### Requirements

Before you can run Kwaak, make sure you have Docker installed on your machine.

Kwaak expects a Dockerfile in the root of your project. This Dockerfile should contain all the dependencies required to test and run your code. Additionally, it expects the following to be present:

- **git**: Required for git operations
- **fd** (https://github.com/sharkdp/fd): Required for searching files. Note that it should be available as `fd`, some systems have it as `fdfind`.
- **ripgrep** (https://github.com/BurntSushi/ripgrep): Required for searching _in_ files. Note that it should be available as `rg`.

If you already have a Dockerfile for other purposes, you can either extend it or provide a new one and override the dockerfile path in the configuration.

_For an example Dockerfile in Rust, see [this projects Dockerfile](/Dockerfile)_

Additionally, you will need an OpenAI API key and a github token.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

### Installation and setup

Currently, Kwaak is not available on crates.io. You can install it by cloning the repository and running `cargo install --path .` in the root of the repository. Proper releases will be available soon.

Once installed, you can run `kwaak --init` in the project you want to use Kwaak in. This will create a `kwaak.toml` in your project root. You can edit this file to configure Kwaak.

After verifying the default configuration, one required step is to set up the `test` and `coverage` commands. There are also some optional settings you can consider.

Api keys can be prefixed by `env:`, `text:` and `file:` to read secrets from the environment, a text string, or a file respectively.

### Running Kwaak

You can then run `kwaak` in the root of your project. This will start the Kwaak terminal interface. On initial bootup, Kwaak will index your codebase. This can take a while, depending on the size of your codebase. Once indexing has been completed once, subsequent startups will be faster.

Keybindings:

- **_ctrl-s_**: Send the current message to the agent
- **_ctrl-x_**: Exit the agent
- **_ctrl-c_**: Exit kwaak
- **_ctrl-n_**: Create a new agent
- **_tab_**: Switch between agents

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Example prompts

- "Identify uncovered code paths, write a test for them, and create a pull request"
- "Add technical documentation to the module `foo`, and create a pull request"
- "Refactor the function `bar` to use a match statement, and create a pull request"
- "Implement a function that calculates the nth fibonacci number, and create a pull request"

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- ROADMAP -->

## Roadmap

- Support for more LLMs
- Tools for code documentation
- Different and specialized agents

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- CONTRIBUTING -->

## Community

If you want to get more involved with `kwaak`, have questions or want to chat, you can find us on [discord](https://discord.gg/3jjXYen9UY).

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Contributing

If you have a great idea, please fork the repo and create a pull request. You can also simply open an issue with the tag "enhancement".

Don't forget to give the project a star! Thanks again!

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
