---
title: Docker Installation
description: Install Faber using Docker
---

# Docker Installation

The easiest way to get started with Faber is using our official Docker image.

## Prerequisites

- Docker 20.10+ with cgroup v2 support
- Linux kernel 5.2+ (for cgroups v2)
- Root or sudo access (for privileged containers)

## Quick Start

Run Faber with a single command:

```bash
docker run --privileged --cgroupns=host -p 3000:3000 vgfractal/faber
```

This will:
- Start Faber on port 3000
- Enable privileged mode for cgroup access
- Use host cgroup namespace

## Custom Image with Tools

The base image includes minimal tooling. For specific use cases, build a custom image:

### C/C++ Compilation

```dockerfile
FROM vgfractal/faber AS faber
FROM debian:latest

RUN apt-get update && apt-get install -y \
    gcc \
    g++ \
    make \
    libc-dev

WORKDIR /opt
COPY --from=faber /opt/faber /opt

EXPOSE 3000/tcp
ENTRYPOINT ["./faber"]
```

Build and run:

```bash
docker build -t my-faber .
docker run --privileged --cgroupns=host -p 3000:3000 my-faber
```

### Python Support

```dockerfile
FROM vgfractal/faber AS faber
FROM python:3.11-slim

WORKDIR /opt
COPY --from=faber /opt/faber /opt

EXPOSE 3000/tcp
ENTRYPOINT ["./faber"]
```

### Node.js Support

```dockerfile
FROM vgfractal/faber AS faber
FROM node:20-slim

WORKDIR /opt
COPY --from=faber /opt/faber /opt

EXPOSE 3000/tcp
ENTRYPOINT ["./faber"]
```

## Docker Compose

For production deployments, use Docker Compose:

```yaml
version: '3.8'

services:
  faber:
    image: vgfractal/faber
    privileged: true
    cgroup: host
    ports:
      - "3000:3000"
    environment:
      - API_KEY=your-secret-api-key
      - CACHE_ENABLED=true
      - RUST_LOG=info
    volumes:
      - /sys/fs/cgroup:/sys/fs/cgroup:rw
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/api/v1/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

Start with:

```bash
docker-compose up -d
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `API_KEY` | (required) | API key for authentication |
| `CACHE_ENABLED` | `false` | Enable request caching |
| `RUST_LOG` | `info` | Log level (error, warn, info, debug, trace) |
| `PORT` | `3000` | HTTP server port |

## Volume Mounts

For persistent data or custom configurations:

```bash
docker run --privileged --cgroupns=host \
  -p 3000:3000 \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  -e API_KEY=your-key \
  vgfractal/faber
```

## Network Configuration

### Expose to Specific Interface

```bash
docker run --privileged --cgroupns=host \
  -p 127.0.0.1:3000:3000 \
  -e API_KEY=your-key \
  vgfractal/faber
```

### Custom Network

```bash
# Create network
docker network create faber-network

# Run Faber
docker run --privileged --cgroupns=host \
  --network faber-network \
  --name faber \
  -e API_KEY=your-key \
  vgfractal/faber
```

## Troubleshooting

### Permission Denied Errors

If you see cgroup permission errors:

```bash
# Pre-create cgroup directory
sudo mkdir -p /sys/fs/cgroup/faber
sudo chmod 777 /sys/fs/cgroup/faber

# Enable controllers
echo "+cpu +memory +pids" | sudo tee /sys/fs/cgroup/faber/cgroup.subtree_control
```

### Port Already in Use

```bash
# Use a different port
docker run --privileged --cgroupns=host \
  -p 8080:3000 \
  -e API_KEY=your-key \
  vgfractal/faber
```

### Check Logs

```bash
docker logs faber-container-name
```

## Production Deployment

For production use:

1. **Use a reverse proxy** (nginx, traefik) for SSL termination
2. **Set a strong API key** via environment variable
3. **Enable request caching** for better performance
4. **Monitor resource usage** with cgroup metrics
5. **Set up health checks** for container orchestration

Example with nginx:

```nginx
server {
    listen 443 ssl http2;
    server_name faber.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

## Next Steps

- [Execute your first task](/getting-started/quick-start/first-task/)
- [Learn about the JavaScript SDK](/sdk/javascript/overview/)
- [Understand security isolation](/core-concepts/isolation/overview/)
