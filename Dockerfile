# Always build against latest stable
ARG RUST_VERSION=1.85-slim
FROM rust:${RUST_VERSION} as builder

# These are needed for kwaak itself to compile and run
RUN apt-get update && apt-get install -y --no-install-recommends \
  ssh curl  \
  libstdc++6 \
  build-essential \
  protobuf-compiler \
  libprotobuf-dev \
  pkg-config libssl-dev iputils-ping \
  make \
  # Needed for copypasta (internal for kwaak)
  libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  # These are needed for kwaak to operate on itself
  git \
  # Then clean up
  && rm -rf /var/lib/apt/lists/*

RUN rustup component add clippy rustfmt
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall cargo-llvm-cov

COPY . /app

WORKDIR /app
