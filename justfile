test:
  RUST_LOG=swiftide=debug RUST_BACKTRACE=1 cargo nextest run --all-features --all-targets

lint:
  cargo clippy --all-features -- -D warnings
  cargo fmt --all -- --check
  typos

lint_fix:
  cargo fmt --all
  cargo fix --all-features --allow-dirty --allow-staged
  typos -w

docker-build:
  docker build -t kwaak .

# Mac and Linux have slightly different behaviour when it comes to git/docker/filesystems.
# This ensures a fast feedback loop on macs.
test-in-docker TEST="": docker-build
  docker volume create kwaak-target-cache
  docker volume create kwaak-cargo-cache
  docker run --rm -it \
      -v /var/run/docker.sock:/var/run/docker.sock \
      -v "$(pwd)":/usr/src/myapp \
      -v kwaak-target-cache:/usr/src/myapp/target \
      -v kwaak-cargo-cache:/usr/local/cargo \
      -w /usr/src/myapp \
      -e RUST_LOG=debug \
      -e RUST_BACKTRACE=1 \
      kwaak \
      bash -c "cargo nextest run --no-fail-fast {{TEST}}"

build-in-docker PROFILE="release": docker-build
  docker volume create kwaak-target-cache
  docker volume create kwaak-cargo-cache
  docker run --rm -it \
      -v /var/run/docker.sock:/var/run/docker.sock \
      -v "$(pwd)":/usr/src/myapp \
      -v kwaak-target-cache:/usr/src/myapp/target \
      -v kwaak-cargo-cache:/usr/local/cargo \
      -w /usr/src/myapp \
      -e RUST_LOG=debug \
      -e RUST_BACKTRACE=1 \
      kwaak \
      bash -c "cargo build --profile {{PROFILE}}"

benchmark-swe INSTANCE="":
  cd benchmarks/swe && uv run kwaak-bench-swe {{ if INSTANCE != "" {"--instance " + INSTANCE } else { ""} }}

# Generate answers using kwaak with RAGAS evaluation
ragas-generate:
  # Run kwaak with RAGAS evaluation to generate answers
  RUST_LOG=debug cargo run --features evaluations -- --allow-dirty eval ragas -i benchmarks/ragas/datasets/kwaak.json --output=benchmarks/ragas/results/kwaak_ragas_answers.json
  # Copy the results file to the datasets directory
  cd benchmarks/ragas && cp results/kwaak_ragas_answers.json datasets/kwaak_answers.json

# Run the RAGAS benchmark on the generated answers
ragas-benchmark:
  # Run RAGAS benchmark on the generated answers
  cd benchmarks/ragas && uv run kwaak-bench-ragas --dataset kwaak_answers

# Run only the faithfulness benchmark (for development)
ragas-faithfulness:
  # Run only the faithfulness metric benchmark
  cd benchmarks/ragas && uv run kwaak-bench-ragas --dataset kwaak_answers --metrics faithfulness

# Run the complete RAGAS pipeline (generate + benchmark)
ragas: ragas-generate ragas-benchmark
