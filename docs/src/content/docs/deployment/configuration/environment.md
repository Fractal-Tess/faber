---
title: Environment Variables
description: Configuration via environment variables
---

# Environment Variables

Configure Faber using environment variables.

## Required Variables

### API_KEY

API key for authentication.

```bash
API_KEY=your-secret-api-key
```

## Optional Variables

### CACHE_ENABLED

Enable request caching.

```bash
CACHE_ENABLED=true
```

Default: `false`

### RUST_LOG

Log level filter.

```bash
RUST_LOG=info
```

Levels:
- `error` - Only errors
- `warn` - Warnings and errors
- `info` - Info, warnings, errors (default)
- `debug` - Debug and above
- `trace` - All messages

### PORT

HTTP server port.

```bash
PORT=3000
```

Default: `3000`

## Docker Example

```bash
docker run --privileged --cgroupns=host \
  -p 3000:3000 \
  -e API_KEY=your-secret-key \
  -e CACHE_ENABLED=true \
  -e RUST_LOG=debug \
  vgfractal/faber
```

## Docker Compose

```yaml
services:
  faber:
    image: vgfractal/faber
    environment:
      - API_KEY=${FABER_API_KEY}
      - CACHE_ENABLED=true
      - RUST_LOG=info
      - PORT=3000
```

## Kubernetes

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: faber-secret
type: Opaque
stringData:
  api-key: your-secret-key
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: faber
spec:
  template:
    spec:
      containers:
      - name: faber
        image: vgfractal/faber
        env:
        - name: API_KEY
          valueFrom:
            secretKeyRef:
              name: faber-secret
              key: api-key
        - name: CACHE_ENABLED
          value: "true"
```

## See Also

- [Docker Deployment](/deployment/docker/setup/) - Deployment guide
- [Authentication](/api/authentication/overview/) - API authentication
