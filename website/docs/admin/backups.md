# Backups & upgrades

## What to back up

=== "SQLite (default)"

    Back up the whole **data directory** (`/data` in Docker, `LARDER_DATA_DIR` otherwise):

    | Path | Contains |
    |---|---|
    | `larder.db` (+ `larder.db-wal`, `larder.db-shm` while running) | Everything except photos: users, households, recipes, meal plans, lists, invites, sessions, import history |
    | `images/` | All photos, as `images/<xx>/<uuid>/{thumb,medium,full}.jpg` |
    | `jwt_secret` | The token-signing secret (unless you set `LARDER_JWT_SECRET`) |

=== "PostgreSQL"

    Back up two things:

    - **The database**, with `pg_dump`.
    - **The data directory**: `images/` and `jwt_secret`.

Losing `jwt_secret` only signs everyone out (a new one is generated); losing `images/` loses
all photos.

### Consistent SQLite backups

The database runs in WAL mode, so copying `larder.db` alone while the server writes can give
an inconsistent copy. Either stop the server, or use SQLite's online backup:

```bash
# Docker: stop briefly and copy the volume
docker compose stop larder
docker run --rm -v larder_larder-data:/data -v "$PWD":/backup debian:bookworm-slim \
  tar czf /backup/larder-$(date +%F).tar.gz -C /data .
docker compose start larder
```

```bash
# Bare metal, while running (needs the sqlite3 tool)
sqlite3 /var/lib/larder/larder.db ".backup '/backups/larder.db'"
rsync -a /var/lib/larder/images /var/lib/larder/jwt_secret /backups/
```

The volume name is `<project>_larder-data`; check with `docker volume ls`.

### PostgreSQL

```bash
docker compose exec -T postgres pg_dump -U larder -Fc larder > larder-$(date +%F).dump
docker run --rm -v larder_larder-data:/data -v "$PWD":/backup debian:bookworm-slim \
  tar czf /backup/larder-data-$(date +%F).tar.gz -C /data .
```

Restore with `pg_restore -U larder -d larder --clean larder-YYYY-MM-DD.dump` and by
extracting the data archive back into the volume.

## The JSON export

Every user can also download **Settings → Export all as JSON backup**: a zip with
`recipes.json`, `manifest.json` and all photos of their household, re-importable into any
Larder server or standalone app. It is a good extra safety net and the way to move recipes
between servers, but it does **not** contain users, meal plans or shopping lists, so it is
not a substitute for a server backup. See [Backup & restore](../guide/backup.md).

The API equivalent is `GET /api/export/json` with a user's token.

## Upgrading

1. **Back up** as above.
2. Read the release notes on GitHub.
3. Update:

    === "Docker Compose"

        ```bash
        docker compose pull
        docker compose up -d
        ```

        To pin a version, use `image: ghcr.io/larrydewey/larder:0.1.0` instead of `latest`.

    === "Binary"

        Replace the `larder` binary and the `web/` directory with the new release, then
        restart the service.

4. On start the server applies any new database migrations automatically, then marks import
   jobs that were running before the restart as failed.
5. Check the logs for `listening on http://…` and open the app. Clients pick up the new web
   app on reload; update the Android APK as well (see
   [Install the Android app](../getting-started/android.md#install-it)).

!!! warning "No downgrades"

    Migrations only go forward. To go back to an older version, restore the backup you made
    before upgrading.

## Restoring onto a new machine

Install Larder, stop it, put the data directory (and the PostgreSQL dump, if used) in place
with the same `LARDER_DATABASE_URL`, make the directory writable by the server's user
(uid 10001 in Docker), and start it. Keep `LARDER_PUBLIC_URL` the same if you use SSO.
