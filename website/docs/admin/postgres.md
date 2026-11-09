# PostgreSQL

Larder runs on SQLite by default, which is plenty for a household. PostgreSQL is supported
for people who already run it or prefer it. Both get the same schema and features; the server
chooses the driver from the database URL.

## Configuring

```bash
LARDER_DATABASE_URL=postgres://larder:secret@db.example.com:5432/larder
```

- `postgres://` and `postgresql://` are both accepted, with any connection parameters
  PostgreSQL clients understand (for example `?sslmode=require`).
- The database and user must exist. Larder creates its tables on first start and migrates them
  on later starts.
- Larder appends `statement-cache-capacity=0` to the URL to work around a type-handling issue
  in its database layer; you do not need to add it.
- Up to 16 connections are opened.

With Docker Compose, use the shipped overlay; see
[Install the server](../getting-started/server.md):

```bash
docker compose -f docker-compose.yml -f docker-compose.postgres.yml up -d
```

It runs `postgres:17-alpine` with user, password and database `larder` (set
`POSTGRES_PASSWORD` in `.env`), stores the data in the `larder-postgres` volume, and starts
Larder only after PostgreSQL reports healthy.

!!! note "The data directory is still needed"

    Photos and the generated `jwt_secret` are always files in `LARDER_DATA_DIR`. Back up both
    the database and that directory.

## SQLite details

For comparison, the SQLite database is opened in WAL mode with foreign keys enabled, a
10-second busy timeout, `synchronous=NORMAL` and up to 8 connections. The file and its parent
directory are created if missing. Use a local disk, not a network share.

## Moving between SQLite and PostgreSQL

There is no built-in database migration tool. To move a household's recipes:

1. In the old server, each household exports **Settings → Export all as JSON backup**.
2. Start the new server with the other database and create the accounts again (the first
   becomes admin; invite the others).
3. Import each backup with **Import → File → Larder backup**.

This moves recipes, photos and the importing user's favourites. Meal plans, shopping lists,
other users' favourites and import history are not carried over.
