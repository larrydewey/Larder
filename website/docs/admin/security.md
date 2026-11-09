# Security

This page describes how Larder protects accounts and data, and what you should know when
exposing it to the internet.

## Passwords

- Passwords are hashed with **Argon2** (the `argon2` crate's default, Argon2id) with a random
  salt per password. Plain passwords are never stored or logged.
- Minimum length is 8 characters (maximum 1024 bytes).
- Sign-in accepts the username or the email address, case-insensitively. A failed sign-in for
  an unknown user takes as long as one for a known user, so response times do not reveal which
  usernames exist. The error is always "invalid username or password".
- There is **no built-in rate limiting** or lockout. If the server is reachable from the
  internet, consider rate limiting `/api/auth/login` in your reverse proxy, or use SSO.
- There is no self-service password reset by email. Admins reset passwords in
  **Settings → Server users**.

## Tokens and sessions

| Token | Format | Lifetime | Storage on the server |
|---|---|---|---|
| Access token | JWT, HS256, signed with `LARDER_JWT_SECRET` / `jwt_secret` | 15 minutes | Not stored |
| Refresh token | 64 random hex characters (32 bytes) | 60 days | Only its SHA-256 hash |

- Every API call sends the access token as `Authorization: Bearer …`. When it expires the app
  uses the refresh token to get a new pair. **Refresh tokens are rotated**: each one works
  once, and using it revokes it. Expired refresh tokens are cleaned up at sign-in.
- **Sign out** revokes the device's refresh token.
- **Changing your password** revokes all of your other refresh tokens; the app sends its own
  refresh token with the change (`refresh_token` in `PATCH /api/users/me`), so the device
  you change it on stays signed in. An admin **Reset password** and **Disable account**
  revoke all of that user's refresh tokens. Access tokens already issued stay valid until they expire (at
  most 15 minutes), except that a disabled user is rejected on every request immediately.
- Every request re-reads the user from the database, so deleted users are locked out at once.
- On the device, tokens are kept in the app's local database (Android app storage, or the
  browser's IndexedDB for the web app). They are not encrypted at rest beyond what the
  platform provides.
- Rotating the signing secret (deleting `jwt_secret` or changing `LARDER_JWT_SECRET`)
  invalidates all access tokens. Refresh tokens remain valid, so apps silently get new access
  tokens.

## Photos are capability URLs

Recipe photos are served from `/api/images/<id>` **without authentication**, so they can be
used directly in `<img>` tags and cached by browsers. The id is a random UUID v4 (122 random
bits), so photos cannot be found by guessing, but **anyone who has a photo's URL can view it**,
for example if it is shared or appears in a log. Photo URLs are sent with
`Cache-Control: public, max-age=31536000, immutable`.

When a photo is replaced, removed or its recipe deleted, the files are deleted and the old URL
stops working (cached copies in browsers may remain).

Uploaded photos are decoded and re-encoded as JPEG, which strips embedded metadata such as
EXIF and GPS location. Images over 80 megapixels are rejected to prevent memory exhaustion.

All other API responses are sent with `Cache-Control: no-store`.

## Households isolate data

Every API query is scoped to the caller's household: one household cannot read or change
another's recipes, plans, lists, photos-by-recipe or import jobs. Server admins manage
accounts but have no API access to other households' content.

## Importing from URLs

Everything the server fetches on a user's behalf (website imports, the photos those pages and
export files link to, and the Mealie server for a live import) goes through a **guarded HTTP
client** (`netguard.rs`). It refuses to connect to:

- loopback addresses (`127.0.0.0/8`, `::1`) and the host names `localhost` and `*.localhost`;
- unspecified addresses (`0.0.0.0`, `::`) and the whole `0.0.0.0/8` range;
- link-local addresses (`169.254.0.0/16`, which includes cloud metadata services such as
  `169.254.169.254`, and `fe80::/10`).

IPv4 addresses written as IPv4-mapped IPv6 (`::ffff:127.0.0.1`) are treated as the IPv4
address. The check is applied:

- to the URL you enter, before anything is fetched ("this address cannot be imported from");
- to **every DNS resolution**: blocked addresses are dropped from the result, and the
  connection fails if none remain, so DNS tricks that resolve a public name to a local address
  do not work;
- to **every redirect hop** (at most 8 redirects), so a public page cannot redirect the server
  to a local address;
- to image URLs found in imported pages or files (blocked ones are skipped and the recipe is
  imported without a photo).

**Private LAN addresses are allowed on purpose** (`10.0.0.0/8`, `172.16.0.0/12`,
`192.168.0.0/16`, unique-local IPv6), because many self-hosters import from other services on
their own network. Only `http` and `https` are accepted.

!!! note

    Any signed-in user can make the server fetch URLs on your LAN. If that matters in your
    environment, restrict the server's outbound network access (for example with a firewall or
    a separate Docker network).

The guard applies only to user-supplied URLs. Requests to your configured OIDC provider and AI
provider use the normal client, so a provider on `localhost` (for example Ollama on
`http://localhost:11434/v1` next to a bare-metal server) keeps working.

In standalone mode the phone fetches URLs itself through the engine, without this guard: it
can only reach what the phone itself can reach.

Outgoing requests identify themselves as
`Mozilla/5.0 (compatible; Larder/0.1; +https://github.com/larrydewey/larder)`, import requests time out
after 20 seconds, and responses are size-limited (10 MB pages, 20 MB images, 20 MB Mealie
JSON).

## Import files

Uploaded export files are processed with protections against malicious archives:

- Request size: `LARDER_MAX_UPLOAD_MB` (200 MB default).
- **Zip bombs**: each entry may decompress to at most 100 MB, and one upload (including
  nested zips, as in Tandoor exports) to at most 1 GB in total. The real decompressed size is
  counted, not the size claimed in the zip header. Gzip (Paprika) is limited the same way.
- At most 50,000 entries per archive.
- **Path traversal**: entries with absolute paths or `..` are ignored. Nothing from an
  archive is ever written to disk by the server except re-encoded photos.
- Text fields are cleaned of HTML; nothing from an import is rendered as HTML.

## The PDF renderer

The cookbook is typeset by Typst running inside the server process, with no access to the
file system or network: templates are compiled into the binary, and book content is passed
as JSON data and only ever rendered as plain text, never evaluated as markup.

## AI assist data flow

When AI assist is enabled, **recipe text and photos that users choose to import with AI are
sent to the configured provider** (Anthropic, OpenAI or your OpenAI-compatible endpoint),
along with a fixed instruction prompt. Nothing else is sent. The server's API key never
reaches clients. In standalone mode the user's own key is stored on the phone and the phone
talks to the provider directly. Use Ollama or another self-hosted model to keep data on your
own hardware. See [AI assist](../guide/ai.md#what-is-sent-to-the-provider).

## Single sign-on

OIDC uses the authorization code flow with PKCE (S256), `state` and `nonce`. The ID token
and userinfo are obtained directly from the provider over TLS with the client credentials.
After sign-in, tokens are passed to the app in the URL fragment (never sent to the server in
a request line) and the web app removes them from the address bar. Allowed redirect targets
are limited to the app scheme, the server itself and configured CORS origins.

## Transport and headers

- Larder does not terminate TLS. Use a [reverse proxy](reverse-proxy.md) for HTTPS.
- The Android app allows cleartext HTTP so it can reach LAN servers; do not use plain HTTP
  across the internet.
- CORS is disabled unless `LARDER_CORS_ORIGINS` is set.
- Larder does not set `Content-Security-Policy`, `Strict-Transport-Security` or similar
  headers. Add them in your reverse proxy if you want them.

## Container

The Docker image runs as an unprivileged user (uid 10001) and only needs write access to
`/data`.

## Reporting a vulnerability

Please report security issues privately to the maintainers (for example through GitHub's
private vulnerability reporting on the repository, if it is enabled) rather than in a public
issue.
