# Docker & Compose

## The image

| | |
|---|---|
| Image | `ghcr.io/larrydewey/larder` |
| Tags | `latest` and the release version (for example `0.1.0`), published for every `v*` tag |
| Platforms | `linux/amd64`, `linux/arm64` |
| Base | `debian:bookworm-slim` with CA certificates and time zone data |
| User | `larder` (uid 10001), not root |
| Port | 8080 |
| Volume | `/data` |
| Entrypoint | `larder` |

Environment variables preset in the image:

```text
LARDER_BIND=0.0.0.0:8080
LARDER_DATA_DIR=/data
LARDER_WEB_DIR=/app/web
```

So with no further configuration the server uses SQLite at `/data/larder.db`, photos in
`/data/images` and the secret in `/data/jwt_secret`, and serves the web UI from `/app/web`.

### Health check

The image declares:

```dockerfile
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s CMD ["larder", "healthcheck"]
```

`larder healthcheck` connects to `127.0.0.1` on the port from `LARDER_BIND`, requests
`GET /healthz` and exits non-zero unless the answer is `200 ok`. The image contains no `curl`,
so use this subcommand in your own orchestrator's checks too. `/healthz` needs no
authentication.

## Compose

See [Install the server](../getting-started/server.md) for the SQLite and PostgreSQL Compose
files. A complete example with HTTPS, SSO and AI assist:

```yaml title="docker-compose.yml"
services:
  larder:
    image: ghcr.io/larrydewey/larder:latest
    restart: unless-stopped
    ports:
      - "127.0.0.1:8080:8080"   # only reachable through the reverse proxy
    volumes:
      - larder-data:/data
    environment:
      LARDER_PUBLIC_URL: https://recipes.example.com
      LARDER_MAX_UPLOAD_MB: "300"
      LARDER_OIDC_ISSUER: https://auth.example.com/application/o/larder
      LARDER_OIDC_CLIENT_ID: larder
      LARDER_OIDC_CLIENT_SECRET: ${OIDC_SECRET}
      LARDER_OIDC_NAME: Authentik
      LARDER_LLM_PROVIDER: anthropic
      LARDER_LLM_API_KEY: ${ANTHROPIC_API_KEY}

volumes:
  larder-data:
```

### Bind mounts instead of a named volume

If you prefer a host directory, make it writable by uid 10001:

```bash
mkdir -p /srv/larder && chown 10001 /srv/larder
```

```yaml
    volumes:
      - /srv/larder:/data
```

Otherwise the server fails at start-up with a "creating data dir" or "writing
/data/jwt_secret" permission error.

### Local AI with Ollama

```yaml
services:
  larder:
    environment:
      LARDER_LLM_PROVIDER: ollama
      LARDER_LLM_BASE_URL: http://ollama:11434/v1
      LARDER_LLM_MODEL: llama3.2-vision
  ollama:
    image: ollama/ollama
    volumes:
      - ollama:/root/.ollama
volumes:
  ollama:
```

Pull the model once with `docker compose exec ollama ollama pull llama3.2-vision`.

### Extra fonts for the cookbook

Mount a font directory and point `LARDER_COOKBOOK_FONT_DIRS` at it:

```yaml
    volumes:
      - ./fonts:/fonts:ro
    environment:
      LARDER_COOKBOOK_FONT_DIRS: /fonts
```

## Updating

```bash
docker compose pull
docker compose up -d
```

Database migrations run automatically at start-up. Back up first; see
[Backups & upgrades](backups.md).

## Logs

```bash
docker compose logs -f larder
```

On start the server logs the database kind, data directory, web directory (or "disabled"),
whether OIDC is on and the AI provider and model, then `listening on http://0.0.0.0:8080`.
