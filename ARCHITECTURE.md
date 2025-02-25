# Architecture

Kwaak has a lightweight, ratatui based frontend that interacts with a backend through dispatched commands. A command includes a channel to feed back responses into.

When kwaak starts, by default it will index the repository using Swiftide (for RAG). After indexation is done, the TUI is started.

When starting the initial chat, a docker container is build, initial context is generated (RAG), the environment is configured, and then the agent is started. A chat corresponds to a session in the backend. Chats are sessions, a session does not have to be a chat.

Agents run in a continuous feedback loop with an LLM until their task is completed.

<img src="https://github.com/bosun-ai/kwaak/blob/master/images/architecture.svg" alt="Architecture">

## Sessions

Sessions in Kwaak represent the abstract state of an ongoing agent interaction. Each chat in the UI corresponds to a session in the backend, but the concept of a session is more general and could potentially be used for non-chat interactions in the future.

### Session Lifecycle

1. **Creation**: When a user starts a new chat or creates a new agent, a new session is created with a unique UUID.
2. **Initialization**: The session sets up a Docker container, generates initial context using RAG, and configures the environment.
3. **Agent Execution**: The session starts an agent (e.g., coding agent or plan-and-act agent) which interacts with the LLM.
4. **Message Handling**: Sessions maintain communication channels for handling messages, including potential agent swaps.
5. **Termination**: When a chat is closed or when Kwaak exits, the associated session is stopped.

### Session Management

Kwaak supports running multiple sessions in parallel, with each session having its own:
- Docker container for sandboxed code execution
- Agent state and configuration
- Communication channels
- Cancellation tokens for managing lifecycle

Users can create new agents (sessions) with `Ctrl-n` and switch between them using the `Tab` key.

### Session Architecture

Sessions are implemented using the following key components:

- `Session`: Core struct representing an ongoing agent interaction
- `RunningSession`: Manages the active state of a session, including the agent, tools, and environment
- `SessionMessage`: Communication mechanism for inter-session messaging

The core idea is that sessions can have different configurations of agents (and tools!), such that new configurations can be experimented with without impacting existing users.

The session configuration is provided in the config.

## Debugging

Kwaak has otel support and includes preconfigured jaeger in the `compose.yml` file. If you set `otel_enabled=true` in the config, traces will appear in jaeger.
