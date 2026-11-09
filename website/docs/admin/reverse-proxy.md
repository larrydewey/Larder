# Reverse proxy & HTTPS

Larder speaks plain HTTP on one port and does not terminate TLS itself. To reach it securely
from outside your network, put it behind a reverse proxy that handles HTTPS.

What the proxy needs to know:

- **Plain HTTP/1.1 is enough.** Larder uses no WebSockets or server-sent events; the app polls
  (sync, import progress). No upgrade headers are needed.
- **Upload size.** Import files and photos are sent in one request. Larder accepts up to
  `LARDER_MAX_UPLOAD_MB` (default 200 MB); make the proxy's limit at least as large.
- **Timeouts.** Generating a large PDF cookbook happens within a single request and can take
  more than a minute for hundreds of recipes with photos. Allow a few minutes. Bulk imports
  return immediately and run in the background.
- **The root path.** Serve Larder at the root of its own host name (for example
  `https://recipes.example.com/`). The web app and the OIDC callback (`/oidc-callback`) assume
  the root; serving under a sub-path is not supported.
- **`LARDER_PUBLIC_URL`** must be the public `https://` address. It is used to build the OIDC
  redirect URI and to check where the web app may be redirected after sign-in. Larder does
  not read `X-Forwarded-*` headers, so this variable is the only way it learns its public
  address.
- **Compression** is done by Larder itself (gzip and others, as the client accepts); the
  proxy can pass responses through.

Then set:

```bash
LARDER_PUBLIC_URL=https://recipes.example.com
```

and, ideally, publish the container port only on `127.0.0.1` so it is reachable only through
the proxy.

=== "Caddy"

    Caddy gets and renews certificates automatically, has no request body limit by default
    and does not time out slow upstream responses by default, so a minimal configuration
    works. The `request_body` block just makes the limit match Larder's:

    ```caddy title="Caddyfile"
    recipes.example.com {
        request_body {
            max_size 200MB
        }
        reverse_proxy 127.0.0.1:8080
    }
    ```

    In Docker, use the service name instead: `reverse_proxy larder:8080`.

=== "nginx"

    nginx limits request bodies to 1 MB by default, which breaks imports and photo uploads,
    and times out upstream responses after 60 seconds.

    ```nginx title="/etc/nginx/sites-available/larder"
    server {
        listen 443 ssl;
        http2 on;
        server_name recipes.example.com;

        ssl_certificate     /etc/letsencrypt/live/recipes.example.com/fullchain.pem;
        ssl_certificate_key /etc/letsencrypt/live/recipes.example.com/privkey.pem;

        client_max_body_size 200m;

        location / {
            proxy_pass http://127.0.0.1:8080;
            proxy_http_version 1.1;
            proxy_set_header Host $host;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
            proxy_read_timeout 300s;
            proxy_send_timeout 300s;
            proxy_request_buffering off;
        }
    }

    server {
        listen 80;
        server_name recipes.example.com;
        return 301 https://$host$request_uri;
    }
    ```

    The forwarded headers are not used by Larder but are useful in your logs.

=== "Traefik"

    Traefik has no request body limit unless you add the `buffering` middleware. With
    Docker labels:

    ```yaml title="docker-compose.yml"
    services:
      larder:
        image: ghcr.io/larrydewey/larder:latest
        restart: unless-stopped
        volumes:
          - larder-data:/data
        environment:
          LARDER_PUBLIC_URL: https://recipes.example.com
        labels:
          - traefik.enable=true
          - traefik.http.routers.larder.rule=Host(`recipes.example.com`)
          - traefik.http.routers.larder.entrypoints=websecure
          - traefik.http.routers.larder.tls.certresolver=letsencrypt
          - traefik.http.services.larder.loadbalancer.server.port=8080
    volumes:
      larder-data:
    ```

    If your entry point sets `respondingTimeouts`, keep `readTimeout` long enough for large
    uploads; the default allows them.

## Android over HTTP

The Android app allows plain `http://` so it can talk to servers on your home network. Over
the internet, always use HTTPS: tokens and passwords travel in every request.

## Using the API from another origin

Browsers block cross-origin API calls unless the origin is listed in `LARDER_CORS_ORIGINS`
(comma-separated, exact origins such as `https://app.example.com`). The Larder web app served
by the server itself does not need this. Native clients such as the Android app are not
affected by CORS.
