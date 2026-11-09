# Configuration reference

All settings are environment variables, read once at start-up. Empty values are treated as
unset and surrounding whitespace is ignored. Boolean variables accept `1`, `true`, `yes` or
`on` (any case) as true; anything else is false. `larder --help` prints a summary.

## Server

| Variable | Default | Example | Description |
|---|---|---|---|
| `LARDER_BIND` | `0.0.0.0:8080` | `127.0.0.1:3000` | Address and port to listen on. |
| `LARDER_PUBLIC_URL` | `http://localhost:<port from LARDER_BIND>` | `https://recipes.example.com` | The external URL users reach the server at, without a trailing slash (one is removed). Used to build the OIDC callback URL and to decide which web redirect targets are allowed. |
| `LARDER_DATA_DIR` | `./data` (`/data` in the Docker image) | `/var/lib/larder` | Directory for photos (`images/`), the generated signing secret (`jwt_secret`) and, by default, the SQLite database. Created if it does not exist. |
| `LARDER_DATABASE_URL` | `sqlite://$LARDER_DATA_DIR/larder.db` | `postgres://larder:secret@db:5432/larder` | `sqlite://…` or `postgres://…` / `postgresql://…`. `DATABASE_URL` is used if this is unset. See [PostgreSQL](postgres.md). |
| `LARDER_WEB_DIR` | `./web` if `./web/index.html` exists, else none (`/app/web` in the Docker image) | `/opt/larder/web` | Flutter web build to serve. Without it the server only serves the API. Unknown paths fall back to `index.html` so app routes work on reload. |
| `LARDER_JWT_SECRET` | generated | `$(openssl rand -hex 48)` | Secret used to sign access tokens (HS256). If unset, a random 96-hex-character secret is generated on first start and saved as `$LARDER_DATA_DIR/jwt_secret` (mode 600). A stored secret shorter than 32 characters is replaced. Changing it invalidates all current access tokens; apps then get new ones with their refresh token, so nobody is signed out. |
| `LARDER_PUBLIC_SHARING` | `true` | `false` | Let household members publish recipes at public share links (`/s/<key>`) that anyone with the link can open. When `false`, no new links can be made and existing links stop working (they work again if it is turned back on). |
| `LARDER_ALLOW_REGISTRATION` | `false` | `true` | Allow anyone to create an account without an invite code; each gets a new household as owner. Invite codes and the very first account always work. Does not affect OIDC auto-registration. |
| `LARDER_CORS_ORIGINS` | empty | `http://localhost:5000,https://app.example.com` | Comma-separated extra origins allowed to call the API from a browser (CORS), for example a Flutter dev server. Also allowed as OIDC web redirect targets. |
| `LARDER_MAX_UPLOAD_MB` | `200` | `500` | Maximum request body size for `/api` in megabytes (imports, photos). A non-numeric value falls back to 200. |
| `RUST_LOG` | `info,tower_http=warn,sqlx=warn` | `debug` | Log filter ([`tracing` syntax](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html)). Logs go to standard output. |

## Single sign-on (OIDC)

OIDC is enabled when **both** `LARDER_OIDC_ISSUER` and `LARDER_OIDC_CLIENT_ID` are set. See
[Single sign-on](oidc.md).

| Variable | Default | Example | Description |
|---|---|---|---|
| `LARDER_OIDC_ISSUER` | – | `https://auth.example.com/application/o/larder` | Issuer URL. `/.well-known/openid-configuration` is appended for discovery (cached for an hour). A trailing `/` is removed. |
| `LARDER_OIDC_CLIENT_ID` | – | `larder` | Client ID. |
| `LARDER_OIDC_CLIENT_SECRET` | – | `long-random-secret` | Client secret, for a confidential client. Omit for a public client (PKCE is always used). |
| `LARDER_OIDC_NAME` | `SSO` | `Authentik` | Shown on the button: "Sign in with Authentik". |
| `LARDER_OIDC_SCOPES` | `openid profile email` | `openid profile email groups` | Requested scopes, space-separated. |
| `LARDER_OIDC_AUTO_REGISTER` | `true` | `false` | Create an account for a new OIDC user. When false, only users already linked (or linkable by verified email) can sign in. |
| `LARDER_OIDC_USERNAME_CLAIM` | `preferred_username` | `nickname` | Userinfo claim used as the username for new accounts. |
| `LARDER_OIDC_ADMIN_GROUP` | – | `larder-admins` | Users whose `groups` claim (from the userinfo endpoint) contains this value become server admins. |

## AI assist

See [AI assist](../guide/ai.md). Admins can also set the provider and key in the app (**Settings → Household → AI
assistant**) without these variables; when `LARDER_LLM_PROVIDER` is set, the variables win and the app
shows them read-only.

| Variable | Default | Example | Description |
|---|---|---|---|
| `LARDER_LLM_PROVIDER` | – (disabled) | `anthropic` | `anthropic` (alias `claude`), `openai` (alias `openai-compatible`), `ollama`, or `none` / `off`. Any other value stops the server from starting. |
| `LARDER_LLM_API_KEY` | – | `sk-ant-…` | API key. Falls back to `ANTHROPIC_API_KEY` (anthropic) or `OPENAI_API_KEY` (openai). Not needed for Ollama. |
| `LARDER_LLM_MODEL` | `claude-opus-5-5` (anthropic), `gpt-4o-mini` (openai), `llama3.2-vision` (ollama) | `gpt-4o` | Model name. |
| `LARDER_LLM_BASE_URL` | `https://api.anthropic.com`, `https://api.openai.com/v1`, `http://localhost:11434/v1` | `http://ollama:11434/v1` | API base URL. For OpenAI-compatible providers, `/chat/completions` is appended; for Anthropic, `/v1/messages`. |

## Cookbook

| Variable | Default | Example | Description |
|---|---|---|---|
| `LARDER_COOKBOOK_FONT_DIRS` | – | `/usr/share/fonts/opentype/noto:/fonts` | `:`-separated directories with extra `.ttf`, `.otf`, `.ttc` or `.otc` fonts for the PDF cookbook (for example CJK fonts). Not recursive. Read when the first cookbook is rendered. See [Fonts and languages](../guide/cookbook.md#fonts-and-languages). |

## Testing and development only

| Variable | Used by |
|---|---|
| `LARDER_TEST_POSTGRES_URL` | `cargo test`: runs the API test scenario against this PostgreSQL database too. **The database is wiped.** |
| `LARDER_ENGINE_LIB` | `flutter test`: path to a host build of `liblarder_engine.so` for the engine integration tests. |
| `LARDER_IMPORT_SAMPLES` | An ignored import test that parses every file in this directory. |
| `COOKBOOK_PERF_RECIPES` | Number of recipes in the cookbook performance test (default 200). |

See [Testing](../development/testing.md).

## Fixed values

These are not configurable:

| Setting | Value |
|---|---|
| Access token lifetime | 15 minutes |
| Refresh token lifetime | 60 days, rotated on every use |
| OIDC login attempt lifetime | 10 minutes |
| Stored photo sizes | 320, 960 and 2400 px (longest edge), JPEG |
| Largest accepted photo | 80 megapixels |
| Outgoing request timeout | 30 s (20 s for page and image downloads during imports) |
| Database pool | 8 connections (SQLite), 16 (PostgreSQL) |

## Example `.env` for Docker Compose

```bash title=".env"
LARDER_PUBLIC_URL=https://recipes.example.com
LARDER_ALLOW_REGISTRATION=false
POSTGRES_PASSWORD=change-me
```

Only `LARDER_PUBLIC_URL`, `LARDER_ALLOW_REGISTRATION` and `POSTGRES_PASSWORD` are passed
through by the shipped Compose files. Add other variables to the `environment:` section of
the `larder` service.
