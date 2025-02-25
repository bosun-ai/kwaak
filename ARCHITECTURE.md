# Architecture

Kwaak has a lightweight, ratatui based frontend that interacts with a backend through dispatched commands. A command includes a channel to feed back responses into.

When kwaak starts, by default it will index the repository using Swiftide (for RAG). After indexation is done, the TUI is started.

When starting the initial chat, a docker container is build, initial context is generated (RAG), the environment is configured, and then the agent is started. A chat corresponds to a session in the backend. Chats are sessions, a session does not have to be a chat.

Agents run in a continuous feedback loop with an LLM until their task is completed.

<img src="https://github.com/bosun-ai/kwaak/blob/master/images/architecture.svg" alt="Architecture">

## Sessions

The core idea is that sessions can have different configurations of agents (and tools!), such that new configurations can be experimented with without impacting existing users.

The session configuration is provided in the config.

## Debugging

Kwaak has otel support and includes preconfigured jaeger in the `compose.yml` file. If you set `otel_enabled=true` in the config, traces will appear in jaeger.
