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

[working-directory: 'benchmarks/swe']
benchmark-swe INSTANCE="":
  uv run kwaak-bench-swe {{ if INSTANCE != "" {"--instance " + INSTANCE } else { ""} }}

[working-directory: 'benchmarks/ragas']
ragas:
  # Step 1: Run kwaak with RAGAS evaluation to generate answers
  cd ../../ && RUST_LOG=debug cargo run --features evaluations -- --allow-dirty eval ragas -i benchmarks/ragas/datasets/kwaak.json --output=benchmarks/ragas/results/kwaak_ragas_answers.json
  # Step 2: Copy the results file to the datasets directory
  cp results/kwaak_ragas_answers.json datasets/kwaak_answers.json
  # Step 3: Run RAGAS benchmark on the generated answers
  uv run kwaak-bench-ragas --dataset kwaak_answers
