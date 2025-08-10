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
COPY docker/dev/lldb-start.sh /faber/scripts/lldb-start.sh

# Create necessary directories for Faber
RUN mkdir -p /tmp/faber/containers /var/log/faber

WORKDIR /faber

ENTRYPOINT [ "/faber/scripts/lldb-start.sh" ]
