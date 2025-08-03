FROM debian:latest

# Install g++ compiler
RUN apt update && apt install -y g++ && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /opt

# Copy the compiled binary
COPY target/x86_64-unknown-linux-musl/debug/faber /opt/faber

# Expose port
EXPOSE 3000

# Run the application
ENTRYPOINT ["./faber"] 