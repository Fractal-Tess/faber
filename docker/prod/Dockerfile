# --- Builder stage ---
FROM rust:1.89.0-slim AS builder

WORKDIR /faber

# Install build dependencies for Debian
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --release 

FROM debian:bookworm-slim

# Install necessary runtime dependencies
# Note: gcc, g++, make, libc-dev are included for C compilation use cases.
# These increase image size but are required for compiling user-submitted code.
# Security is maintained through namespace isolation, cgroup resource limits,
# and capability dropping - see crates/faber-runtime/src/runtime/core.rs
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    gcc \
    g++ \
    make \
    libc-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary and make it executable
COPY --from=builder /faber/target/release/faber /opt/faber
RUN chmod +x /opt/faber

ENTRYPOINT ["/opt/faber"]
