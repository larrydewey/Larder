# Larder

A recipe manager and cookbook: a fast Rust server with a Flutter client that runs as a
**native Android app** and as the **web UI**, with the same look and behaviour on both. The
Android app also works **standalone, with no server at all**, and can be connected to a server
later.

- **Recipes** with structured ingredients (sections, quantities, ranges, units), steps, photos,
  tags, categories, ratings, notes, nutrition and source links. Search, filters, favourites.
- **Scaling & unit conversion**: scale to any number of servings and show quantities in the
  original units, metric or imperial, with kitchen-friendly fractions (1½ cups).
- **Cook mode**: step-by-step full-screen view that keeps the screen awake, with timers detected
  from the instructions ("bake for 25 minutes").
- **Meal planner** (week view) and **shopping lists** that merge ingredients across recipes,
  scale them to planned servings and group them by supermarket aisle.
- **Import** from Mealie (export/backup zip, or straight from a running Mealie server via its
  API), Tandoor, Paprika, Nextcloud Cookbook, Recipe Keeper, schema.org JSON-LD files, any recipe
  website URL, or pasted text. Optional **AI assist** (Claude, OpenAI or a local Ollama model)
  turns messy text or a **photo of a recipe card** into a structured recipe.
- **Printable cookbook**: export all (or selected) recipes to a typeset PDF with a title page,
  dedication, table of contents, chapters, running headers and an index, in three themes
  (Classic, Modern, Rustic) on Letter, A4 or A5 paper. Rendered by an embedded
  [Typst](https://typst.app) engine, on the server or on the phone.
- **Households**: multi-user accounts grouped into households that share recipes, meal plans
  and lists; invite codes; household owners and server admins.
- **Sign-in** with local accounts (argon2 + JWT with refresh-token rotation) and optional
  **OpenID Connect** single sign-on (Authentik, Keycloak, Authelia, Zitadel, ...).
- **Standalone or connected Android app**: choose *Use on this device* and everything (imports,
  shopping-list merging, AI assist with your own API key, the cookbook PDF) runs on the phone
  through the same Rust engine the server uses. Choose *Connect to a server* to share with your
  household. When connected, the app is offline-first: data is cached locally and edits made
  offline are queued and synced when the connection returns. You can **connect a standalone app
  to a server later**; it uploads everything once, can be resumed, and can skip recipes that
  already exist there. You can also disconnect and keep your data on the device.
- **SQLite or PostgreSQL**: chosen by the database URL.
- **Backup**: export everything as a zip (JSON + photos) and re-import it later.

## Repository layout

| Path | What |
|---|---|
| `server/crates/core` | Shared models, ingredient parser, unit conversion, aisle classification |
| `server/crates/import` | Importers: Mealie, Tandoor, Paprika, Nextcloud, Recipe Keeper, JSON-LD, URL scraping, plain text |
| `server/crates/cookbook` | Typst-based printable cookbook PDF renderer and themes |
| `server/crates/assist` | Optional LLM assist (Claude / OpenAI-compatible) |
| `server/crates/engine` | `liblarder_engine.so`: the on-device engine for standalone mode (C ABI, JSON in/out) |
| `server/crates/server` | axum HTTP server (`larder` binary), database, auth, API |
| `app/` | Flutter client (Android + web) |
| `docs/API.md` | HTTP API reference |
| `docs/ENGINE.md` | On-device engine contract |

## Quick start (Docker)

```bash
docker compose up -d            # SQLite, data in the "larder-data" volume
# or with PostgreSQL:
docker compose -f docker-compose.yml -f docker-compose.postgres.yml up -d
```

Open <http://localhost:8080>. The first account you register becomes the administrator. Invite
family members from **Settings → Household → Invite**. To use the Android app, install the APK
(from the GitHub release or the CI artifact) and enter your server's URL on first launch, or pick
**Use on this device** to run it without a server.

## Configuration

All settings are environment variables (also listed by `larder --help`):

| Variable | Default | Description |
|---|---|---|
| `LARDER_BIND` | `0.0.0.0:8080` | Listen address |
| `LARDER_DATABASE_URL` | `sqlite://$DATA_DIR/larder.db` | `sqlite://…` or `postgres://user:pass@host/db` (`DATABASE_URL` is used if this is unset) |
| `LARDER_DATA_DIR` | `./data` (`/data` in Docker) | Photos and the generated signing secret |
| `LARDER_WEB_DIR` | `./web` if it contains `index.html`, else none (`/app/web` in Docker) | Flutter web build to serve; without one the server is API-only |
| `LARDER_PUBLIC_URL` | `http://localhost:8080` | External URL (used for OIDC redirects) |
| `LARDER_JWT_SECRET` | generated | Token signing secret |
| `LARDER_ALLOW_REGISTRATION` | `false` | Allow anyone to sign up (each gets their own household). Invite codes always work. |
| `LARDER_CORS_ORIGINS` | empty | Extra origins allowed to call the API (e.g. a dev web server) |
| `LARDER_MAX_UPLOAD_MB` | `200` | Maximum upload size (imports, photos) |
| `LARDER_OIDC_ISSUER`, `_CLIENT_ID`, `_CLIENT_SECRET` | – | Enable OpenID Connect sign-in |
| `LARDER_OIDC_NAME` | `SSO` | Button label ("Sign in with …") |
| `LARDER_OIDC_SCOPES` | `openid profile email` | Requested scopes |
| `LARDER_OIDC_AUTO_REGISTER` | `true` | Create accounts for new OIDC users |
| `LARDER_OIDC_USERNAME_CLAIM` | `preferred_username` | Claim used as the username |
| `LARDER_OIDC_ADMIN_GROUP` | – | Members of this group (in the `groups` claim) become admins |
| `LARDER_LLM_PROVIDER` | – | `anthropic`, `openai` or `ollama` to enable AI assist |
| `LARDER_LLM_API_KEY` | – | API key (`ANTHROPIC_API_KEY` / `OPENAI_API_KEY` also work) |
| `LARDER_LLM_MODEL` | `claude-opus-5-5` / `gpt-4o-mini` / `llama3.2-vision` (anthropic / openai / ollama) | Model name |
| `LARDER_LLM_BASE_URL` | `https://api.anthropic.com` / `https://api.openai.com/v1` / `http://localhost:11434/v1` | e.g. `http://ollama:11434/v1` |
| `LARDER_COOKBOOK_FONT_DIRS` | – | `:`-separated directories with extra fonts for the PDF cookbook (e.g. CJK) |

### OpenID Connect

Register a confidential client with redirect URI `${LARDER_PUBLIC_URL}/api/auth/oidc/callback`.
The Android app signs in through the system browser and returns via the `larder://auth` scheme.
An existing local account is linked automatically when the provider reports the same, verified
email address.

### AI assist

With `LARDER_LLM_PROVIDER=anthropic` the server uses the Claude Messages API with structured
outputs. Requests go to the default model `claude-opus-5-5` at low effort, which is plenty for
transcription. For models that support it, the server opts into Anthropic's server-side refusal
fallback (`fallbacks: "default"`), so a request a safety classifier declines is retried on a
fallback model instead of failing. `openai` and `ollama` use any OpenAI-compatible chat
completions API, so you can keep everything on your own hardware with a vision-capable local
model.

## Importing

| Source | How to export it | Larder import |
|---|---|---|
| Mealie | *Settings → Data management → Export* (recipe zip) or a backup zip, **or** connect to the live server with an API token (*Profile → API tokens*) | File: Mealie / Mealie server |
| Tandoor | *Export → Default* (zip) | File: Tandoor |
| Paprika | *File → Export → Paprika Recipe Format* (`.paprikarecipes`) | File: Paprika |
| Nextcloud Cookbook | Download the `Recipes` folder as a zip | File: Nextcloud |
| Recipe Keeper | *Settings → Export* (zip) | File: Recipe Keeper |
| Websites | Any page with schema.org recipe markup | URL |
| Anything else | Paste text, or (with AI enabled) take a photo | Text / Photo |

Duplicates (same source URL, or same name ignoring case and accents) are skipped by default.
Recipes without a category are sorted into one of Larder's categories (Breakfast … Household &
DIY), and ingredient lines are tidied into one standard form. Large imports run in the
background and report progress. Existing collections can be tidied and categorised in bulk
with `POST /api/recipes/organize`.

## Development

```bash
# Server
cd server
cargo run -p larder-server --bin larder           # http://localhost:8080
cargo test --workspace                                 # SQLite tests
LARDER_TEST_POSTGRES_URL=postgres://postgres@localhost/larder_test cargo test --workspace

# On-device engine (needed for the Android app; requires cargo-ndk + Android NDK)
app/tool/build_engine.sh            # Android ABIs -> app/android/app/src/main/jniLibs
app/tool/build_engine.sh --host     # Linux .so for `LARDER_ENGINE_LIB=... flutter test`

# App (web): run the server with LARDER_CORS_ORIGINS=http://localhost:5000, then
cd app
flutter run -d chrome --web-port 5000
# App (Android)
flutter run -d <device>
```

The web UI is built with `flutter build web --release` and served by the server from
`LARDER_WEB_DIR`.

## License

MIT, see [LICENSE](LICENSE). Bundled fonts are licensed under the SIL Open Font License (see the
`OFL.txt` files next to them).
