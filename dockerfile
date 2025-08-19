FROM criyle/go-judge:latest AS go-judge 
FROM debian:latest
# install compilers
RUN apt update && apt install -y \
    curl \
    g++ \
    procps \
    lldb \
    gdb \
    build-essential \
    btop

WORKDIR /opt
COPY --from=go-judge /opt/go-judge /opt/mount.yaml /opt/
EXPOSE 5050/tcp 5051/tcp 5052/tcp
ENTRYPOINT ["./go-judge"]