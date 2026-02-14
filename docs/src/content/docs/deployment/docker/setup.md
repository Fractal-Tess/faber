---
title: Docker Deployment
description: Deploy Faber with Docker
---

# Docker Deployment

Deploy Faber in production using Docker and Docker Compose.

## Basic Deployment

Run Faber container:

```bash
docker run -d \
  --name faber \
  --privileged \
  --cgroupns=host \
  -p 3000:3000 \
  -e API_KEY=your-secret-api-key \
  vgfractal/faber
```

## Docker Compose

Production-ready Docker Compose configuration:

```yaml
version: '3.8'

services:
  faber:
    image: vgfractal/faber:latest
    container_name: faber
    privileged: true
    cgroup: host
    ports:
      - "3000:3000"
    environment:
      - API_KEY=${FABER_API_KEY}
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
      start_period: 5s
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 2G
        reservations:
          cpus: '0.5'
          memory: 512M

  # Optional: Redis for external caching
  redis:
    image: redis:7-alpine
    container_name: faber-redis
    restart: unless-stopped
    volumes:
      - redis-data:/data

  # Optional: Nginx reverse proxy
  nginx:
    image: nginx:alpine
    container_name: faber-nginx
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
      - ./ssl:/etc/nginx/ssl:ro
    depends_on:
      - faber
    restart: unless-stopped

volumes:
  redis-data:
```

Environment file (`.env`):

```
FABER_API_KEY=your-secret-api-key-here
```

Start services:

```bash
docker-compose up -d
```

## Nginx Configuration

Reverse proxy with SSL:

```nginx
events {
    worker_connections 1024;
}

http {
    upstream faber {
        server faber:3000;
    }

    server {
        listen 80;
        server_name faber.example.com;
        return 301 https://$server_name$request_uri;
    }

    server {
        listen 443 ssl http2;
        server_name faber.example.com;

        ssl_certificate /etc/nginx/ssl/cert.pem;
        ssl_certificate_key /etc/nginx/ssl/key.pem;
        ssl_protocols TLSv1.2 TLSv1.3;
        ssl_ciphers HIGH:!aNULL:!MD5;

        location / {
            proxy_pass http://faber;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
            
            # Timeouts for long-running tasks
            proxy_connect_timeout 60s;
            proxy_send_timeout 60s;
            proxy_read_timeout 60s;
        }
    }
}
```

## Kubernetes Deployment

Deploy to Kubernetes:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: faber
  labels:
    app: faber
spec:
  replicas: 3
  selector:
    matchLabels:
      app: faber
  template:
    metadata:
      labels:
        app: faber
    spec:
      containers:
      - name: faber
        image: vgfractal/faber:latest
        ports:
        - containerPort: 3000
        env:
        - name: API_KEY
          valueFrom:
            secretKeyRef:
              name: faber-secret
              key: api-key
        - name: CACHE_ENABLED
          value: "true"
        securityContext:
          privileged: true
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /api/v1/health
            port: 3000
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /api/v1/health
            port: 3000
          initialDelaySeconds: 5
          periodSeconds: 10
---
apiVersion: v1
kind: Service
metadata:
  name: faber
spec:
  selector:
    app: faber
  ports:
  - port: 80
    targetPort: 3000
  type: ClusterIP
---
apiVersion: v1
kind: Secret
metadata:
  name: faber-secret
type: Opaque
stringData:
  api-key: your-secret-api-key
```

Apply:

```bash
kubectl apply -f faber-deployment.yaml
```

## Monitoring

### Prometheus Metrics

Faber exposes metrics at `/metrics` (when implemented):

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'faber'
    static_configs:
      - targets: ['faber:3000']
```

### Health Checks

Docker health check:

```dockerfile
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:3000/api/v1/health || exit 1
```

### Logging

View logs:

```bash
# Docker
docker logs -f faber

# Docker Compose
docker-compose logs -f faber

# Kubernetes
kubectl logs -f deployment/faber
```

## Security Best Practices

1. **Use strong API keys** - Generate random 32+ character keys
2. **Enable HTTPS** - Use reverse proxy with SSL
3. **Restrict network access** - Firewall rules, VPC
4. **Run with minimal privileges** - Drop unnecessary capabilities
5. **Monitor resource usage** - Set appropriate limits
6. **Keep images updated** - Regular security patches

## Troubleshooting

### Container Won't Start

Check cgroup setup:

```bash
# Host must have cgroup v2
ls /sys/fs/cgroup/cgroup.controllers

# Pre-create faber cgroup
sudo mkdir -p /sys/fs/cgroup/faber
sudo chmod 777 /sys/fs/cgroup/faber
echo "+cpu +memory +pids" | sudo tee /sys/fs/cgroup/faber/cgroup.subtree_control
```

### Out of Memory

Increase memory limit:

```yaml
deploy:
  resources:
    limits:
      memory: 4G
```

### Slow Responses

Enable caching:

```yaml
environment:
  - CACHE_ENABLED=true
```

## Next Steps

- [Configuration](/deployment/configuration/environment/) - Environment variables
- [API Reference](/api/rest/endpoints/) - API documentation
- [Examples](/examples/patterns/basic/) - Usage examples
