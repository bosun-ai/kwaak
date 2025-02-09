## Evaluating tool and agent contributions

Because of the unpredictable nature of LLM based tools, it is important to verify that changes to tools and agents do
not subtly break. To help with this any improvements should accompanied with an eval that reveals the intended improved
behavior.

There is an example of such an eval in `src/evaluations/patch.rs` which evaluates the performance of patching a file.

An eval can be run as follows:

```shell
cargo run -- -c test-config.toml eval patch -i 5
```

The evals are analogous to unit tests, but statistical instead of deterministic. They should exercise a subset of
the tools and/or agent behavior. By designing the evals this way, they are less expensive than running a full benchmark
such as SWE-bench, but still protect the agent from regressing on intended behavior.