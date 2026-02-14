FROM rust:1.89.0-slim AS builder
WORKDIR /faber
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libssl3 ca-certificates gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /faber/target/release/faber /opt/faber
RUN chmod +x /opt/faber
ENTRYPOINT ["/opt/faber"]
