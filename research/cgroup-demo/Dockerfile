# Stage 1: Build static binary
FROM rust:1.83-alpine AS builder

# Install musl-dev for static linking
RUN apk add --no-cache musl-dev

WORKDIR /app

# Copy source files
COPY Cargo.toml Cargo.lock* ./
COPY src ./src

# Build static release binary
RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Minimal runtime
FROM alpine:3.21

# Install minimal tools for debugging (optional)
RUN apk add --no-cache procps

# Copy the static binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/cgroup-demo /usr/local/bin/cgroup-demo

# Make it executable
RUN chmod +x /usr/local/bin/cgroup-demo

# Run as root (required for cgroup manipulation)
CMD ["/usr/local/bin/cgroup-demo"]
