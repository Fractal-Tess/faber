FROM debian:latest

RUN apt update && apt install -y \
    curl \
    g++ \
    procps \
    lldb \
    gdb \
    build-essential \
    btop

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable && \
    . $HOME/.cargo/env && \
    rustup target add x86_64-unknown-linux-musl

ENV PATH="/root/.cargo/bin:${PATH}"

RUN cargo install cargo-watch

WORKDIR /faber

ENTRYPOINT ["tail", "-f", "/dev/null"]
