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
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary and make it executable
COPY --from=builder /faber/target/release/faber /opt/faber
RUN chmod +x /opt/faber

ENTRYPOINT ["/opt/faber"]
