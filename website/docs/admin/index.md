# Administration

This section is for whoever runs the Larder server.

The server is a single binary, `larder`, configured entirely through environment variables.
It stores data in SQLite or PostgreSQL plus a data directory for photos and the token-signing
secret, serves the Flutter web UI and the JSON API on one port, and needs no other services.

| Page | Covers |
|---|---|
| [Configuration reference](configuration.md) | Every `LARDER_*` variable with its default |
| [Docker & Compose](docker.md) | The image, volumes, health check, upgrades with Compose |
| [PostgreSQL](postgres.md) | Using PostgreSQL instead of SQLite, and moving between them |
| [Reverse proxy & HTTPS](reverse-proxy.md) | Caddy, nginx and Traefik examples |
| [Single sign-on (OIDC)](oidc.md) | Authentik, Keycloak, Authelia, Pocket ID |
| [Users & administration](users.md) | Server admins, household owners, the Server users screen |
| [Backups & upgrades](backups.md) | What to back up and how to upgrade |
| [Security](security.md) | Passwords, tokens, image URLs, imports, AI data flow |

## Checklist for a new server

- [ ] Set `LARDER_PUBLIC_URL` to the address people use.
- [ ] Put the server behind HTTPS if it is reachable from outside your network.
- [ ] Create the admin account immediately after the first start (the first account to
      register becomes admin).
- [ ] Keep `LARDER_ALLOW_REGISTRATION` off unless you want strangers to sign up; use invites.
- [ ] Back up the data directory (and the PostgreSQL database, if used).
- [ ] Optional: single sign-on, AI assist.
