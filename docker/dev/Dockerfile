FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    gcc \
    g++ \
    make \
    libc-dev \
    pkg-config \
    libssl-dev \
    procps \
    lldb \
    gdb \
    build-essential \
    btop \
    && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable && \
    . $HOME/.cargo/env && \
    rustup target add x86_64-unknown-linux-musl

ENV PATH="/root/.cargo/bin:${PATH}"

RUN cargo install cargo-watch

WORKDIR /faber

ENTRYPOINT ["tail", "-f", "/dev/null"]
