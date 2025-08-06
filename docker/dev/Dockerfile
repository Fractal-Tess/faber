FROM debian:latest

RUN apt update && apt install -y \
    curl \
    g++ \
    lldb \
    gdb \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable
RUN rustup target add x86_64-unknown-linux-musl
ENV PATH="/root/.cargo/bin:${PATH}"

RUN cargo install cargo-watch
COPY docker/dev/lldb-start.sh /faber/scripts/lldb-start.sh

WORKDIR /faber

ENTRYPOINT [ "/faber/scripts/lldb-start.sh" ]
