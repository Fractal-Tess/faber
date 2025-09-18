# --- Builder stage ---
FROM rust:1.89.0-alpine AS builder

WORKDIR /faber

# Install build dependencies for Alpine
RUN apk add --no-cache musl-dev

COPY . .

RUN cargo build --release 

FROM alpine:3.19

# Install necessary runtime dependencies
RUN apk add --no-cache libgcc

# Copy the binary and make it executable
COPY --from=builder /faber/target/release/faber /usr/local/bin/faber
RUN chmod +x /usr/local/bin/faber

ENTRYPOINT ["/usr/local/bin/faber"]
