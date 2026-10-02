# Faber with common language toolchains on top of the production image.
# scripts/toolchain-test.sh runs each of them through the default sandbox
# profile; it is also a starting point for images that serve these languages.
ARG BASE=faber-test:latest
FROM ${BASE}

RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 \
    nodejs \
    default-jdk-headless \
    golang-go \
    rustc \
    && rm -rf /var/lib/apt/lists/*

# The JDK keeps its configuration under /etc, which sandboxes do not see by
# default.
ENV SANDBOX_READONLY_PATHS=/bin,/lib,/lib64,/usr,/etc/alternatives,/etc/ld.so.cache,/etc/passwd,/etc/group,/etc/java-17-openjdk
