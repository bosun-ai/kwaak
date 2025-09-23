# Always build against latest stable
ARG RUST_VERSION=1.90
FROM rust:${RUST_VERSION}

# Install rust tools
RUN rustup component add clippy rustfmt
RUN curl -L --proto '=https' --tlsv1.2 -sSf \
  https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh \
  | bash
RUN cargo binstall cargo-llvm-cov

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
  libprotobuf-dev \
  libssl-dev \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libxcb1-dev \
  protobuf-compiler \
  # Faster builds
  lld \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Build test binaries so we can get started right away
ENV RUSTFLAGS="-C link-arg=-fuse-ld=lld"

# Build dependencies so they can be cached in a docker layer
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
  && echo 'fn main() { println!("ERROR in docker build"); }' > src/main.rs \
  && cargo test --no-run \
  && rm -rf src

# Build the actual project
# Build test binaries so we can get started right away
COPY . .
