# Getting started

Larder has two parts:

- **The app**: one Flutter code base that runs as a native **Android app** and as the **web
  UI**. Both look and behave the same.
- **The server** (optional): a single Rust binary, `larder`, that stores your household's data
  in SQLite or PostgreSQL, serves the web UI and the HTTP API, and does the heavy lifting
  (imports, shopping-list merging, PDF rendering, AI assist).

On Android you can skip the server entirely and keep everything on the phone. On the web you
always need a server, because the server is what serves the web app.

## The four steps

1. **[Decide how to run it](choosing.md)**: standalone on one phone, a server for the whole
   household, or both.
2. **[Install the server](server.md)** with Docker Compose (SQLite or PostgreSQL) or as a bare
   binary. Skip this if you only want standalone Android.
3. **[Install the Android app](android.md)** from GitHub Releases. Skip this if you only want
   to use the web UI.
4. **[First steps](first-steps.md)**: create the administrator account, invite your family and
   import your first recipes.

!!! tip "Just want to look around?"

    `docker compose up -d` in a checkout of the repository starts a server on
    <http://localhost:8080>. The first account you register becomes the administrator.
