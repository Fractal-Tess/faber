# --- Builder stage ---
FROM rust:1.89.0-slim-bookworm AS chef

WORKDIR /faber

# Install build dependencies for Debian
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*
ARG CARGO_CHEF_VERSION=0.1.78
RUN cargo install cargo-chef --locked --version "${CARGO_CHEF_VERSION}"

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /faber/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

# Install necessary runtime dependencies
# Note: gcc, g++, make, libc-dev are included for C compilation use cases.
# These increase image size but are required for compiling user-submitted code.
# Security is maintained through namespace isolation, cgroup resource limits,
# and capability dropping - see crates/faber-runtime/src/runtime/core.rs
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 \
    ca-certificates \
    curl \
    iproute2 \
    gcc \
    g++ \
    make \
    libc-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary and make it executable
COPY --from=builder /faber/target/release/faber /opt/faber
RUN chmod +x /opt/faber

ENTRYPOINT ["/opt/faber"]
