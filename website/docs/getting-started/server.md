# Install the server

The server is published as a container image, `ghcr.io/larrydewey/larder`, for `linux/amd64`
and `linux/arm64`. The image contains the `larder` binary and the built web UI, runs as an
unprivileged user (uid 10001), listens on port 8080 and keeps all its data in the `/data`
volume.

## Docker Compose

The repository contains two Compose files: `docker-compose.yml` (SQLite) and
`docker-compose.postgres.yml` (an overlay that switches to PostgreSQL).

=== "SQLite (default)"

    ```yaml title="docker-compose.yml"
    services:
      larder:
        image: ghcr.io/larrydewey/larder:latest
        restart: unless-stopped
        ports:
          - "8080:8080"
        volumes:
          - larder-data:/data
        environment:
          LARDER_PUBLIC_URL: ${LARDER_PUBLIC_URL:-http://localhost:8080}
          LARDER_ALLOW_REGISTRATION: ${LARDER_ALLOW_REGISTRATION:-false}

    volumes:
      larder-data:
    ```

    ```bash
    docker compose up -d
    ```

    The database is `/data/larder.db` inside the `larder-data` volume, next to the photos
    (`/data/images`) and the generated token-signing secret (`/data/jwt_secret`).

=== "PostgreSQL"

    Keep `docker-compose.yml` and add the overlay:

    ```yaml title="docker-compose.postgres.yml"
    services:
      larder:
        environment:
          LARDER_DATABASE_URL: postgres://larder:${POSTGRES_PASSWORD:-larder}@postgres:5432/larder
        depends_on:
          postgres:
            condition: service_healthy

      postgres:
        image: postgres:17-alpine
        restart: unless-stopped
        environment:
          POSTGRES_USER: larder
          POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-larder}
          POSTGRES_DB: larder
        volumes:
          - larder-postgres:/var/lib/postgresql/data
        healthcheck:
          test: ["CMD-SHELL", "pg_isready -U larder -d larder"]
          interval: 5s
          timeout: 5s
          retries: 10

    volumes:
      larder-postgres:
    ```

    ```bash
    echo "POSTGRES_PASSWORD=$(openssl rand -hex 16)" >> .env
    docker compose -f docker-compose.yml -f docker-compose.postgres.yml up -d
    ```

    Photos and the signing secret still live in the `larder-data` volume; only the database
    moves to PostgreSQL. See [PostgreSQL](../admin/postgres.md).

Open <http://localhost:8080> (or your server's address). The first account you register
becomes the server administrator; see [First steps](first-steps.md).

!!! tip "Set `LARDER_PUBLIC_URL`"

    Set `LARDER_PUBLIC_URL` to the address people actually use, for example
    `https://recipes.example.com`, in a `.env` file next to the Compose file. It is required
    for single sign-on and allowed web redirects. See the
    [configuration reference](../admin/configuration.md).

### Building the image yourself

The Compose file also has `build: .`, so in a checkout you can build locally instead of
pulling:

```bash
docker compose build
docker compose up -d
```

The Dockerfile builds the Flutter web UI (Flutter 3.47.9), then the Rust server, and copies
both into a `debian:bookworm-slim` runtime image. See [Docker & Compose](../admin/docker.md)
for details such as the health check.

## Running the bare binary

You can run the server without Docker. Build it from source (see
[Building from source](../development/building.md)) or copy the binary out of the image:

```bash
docker create --name larder-tmp ghcr.io/larrydewey/larder:latest
docker cp larder-tmp:/usr/local/bin/larder ./larder
docker cp larder-tmp:/app/web ./web
docker rm larder-tmp
```

All configuration comes from environment variables. The defaults are designed for running
from a directory that contains `web/` and a `data/` folder:

```bash
export LARDER_PUBLIC_URL=https://recipes.example.com
./larder                       # listens on 0.0.0.0:8080
```

| Default | Meaning |
|---|---|
| `LARDER_BIND=0.0.0.0:8080` | Listen address |
| `LARDER_DATA_DIR=./data` | Created if missing; holds `larder.db`, `images/` and `jwt_secret` |
| `LARDER_DATABASE_URL=sqlite://./data/larder.db` | SQLite next to the data |
| `LARDER_WEB_DIR` unset | `./web` is served if `./web/index.html` exists; otherwise the server is API-only |

Useful commands:

```bash
./larder --help        # lists every environment variable
./larder --version     # prints "larder 0.1.0"
./larder healthcheck   # exits 0 if http://127.0.0.1:<port>/healthz answers 200
```

Without a web build, requests outside `/api` get
`Larder API server. The web UI is not installed (set LARDER_WEB_DIR).`

### A systemd unit

```ini title="/etc/systemd/system/larder.service"
[Unit]
Description=Larder recipe server
After=network-online.target
Wants=network-online.target

[Service]
User=larder
WorkingDirectory=/opt/larder
Environment=LARDER_DATA_DIR=/var/lib/larder
Environment=LARDER_WEB_DIR=/opt/larder/web
Environment=LARDER_PUBLIC_URL=https://recipes.example.com
ExecStart=/opt/larder/larder
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

The server shuts down gracefully on `SIGTERM` and `Ctrl-C`. Logs go to standard output; set
`RUST_LOG` (default `info`) to change the level, for example `RUST_LOG=debug`.

## Database migrations

The schema is created and migrated automatically every time the server starts. There is no
separate migration command. See [Backups & upgrades](../admin/backups.md) before upgrading.

## Next steps

- Put it behind HTTPS: [Reverse proxy & HTTPS](../admin/reverse-proxy.md).
- Enable single sign-on: [OIDC](../admin/oidc.md).
- Enable AI assist: [AI assist](../guide/ai.md).
- [First steps](first-steps.md): create your account and invite your family.
