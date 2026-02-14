---
title: Authentication
description: API authentication methods
---

# Authentication

Faber API uses API key authentication.

## API Key

Set via environment variable:

```bash
API_KEY=your-secret-key
```

## Authentication Methods

### Header Authentication (Recommended)

```
Authorization: Bearer your-api-key
```

Example:

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Authorization: Bearer your-api-key" \
  -H "Content-Type: application/json" \
  -d '[{"cmd": "/bin/echo", "args": ["Hello"]}]'
```

### Alternative Header Format

```
Authorization: your-api-key
```

### Query Parameter

```
?api_key=your-api-key
```

Example:

```bash
curl "http://localhost:3000/api/v1/execute?api_key=your-api-key" \
  -H "Content-Type: application/json" \
  -d '[{"cmd": "/bin/echo", "args": ["Hello"]}]'
```

## Generating API Keys

Generate a secure random key:

```bash
# 32 character key
openssl rand -hex 32

# Base64 encoded
openssl rand -base64 32
```

## Security Best Practices

1. **Use strong keys** - Minimum 32 characters, random
2. **Rotate keys regularly** - Change every 90 days
3. **Use HTTPS** - Never send keys over HTTP
4. **Store securely** - Use secrets management
5. **Limit exposure** - Different keys for different environments

## SDK Usage

```typescript
const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: process.env.FABER_API_KEY!, // From environment
});
```

## Error Response

Invalid or missing key:

```json
{
  "error": "Unauthorized",
  "message": "Invalid or missing API key"
}
```

Status code: 401

## See Also

- [Docker Deployment](/deployment/docker/setup/) - Secure deployment
- [REST API](/api/rest/endpoints/) - API documentation
