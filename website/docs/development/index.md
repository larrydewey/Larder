# Development

Larder is a Rust workspace (the server and the libraries it shares with the phone) plus a
Flutter app. Everything lives in one repository:

| Path | What |
|---|---|
| `server/crates/core` | Shared models, ingredient parser, unit conversion, aisle classification, shopping merge |
| `server/crates/import` | Importers: Mealie, Tandoor, Paprika, Nextcloud, Recipe Keeper, JSON-LD, URL scraping, plain text |
| `server/crates/cookbook` | Typst-based printable cookbook renderer, themes and fonts |
| `server/crates/assist` | Optional LLM assist (Anthropic / OpenAI-compatible) |
| `server/crates/engine` | `liblarder_engine.so`, the on-device engine for standalone mode (C ABI, JSON in and out) |
| `server/crates/server` | The axum HTTP server (`larder` binary), database, auth, API |
| `app/` | The Flutter client (Android + web) |
| `docs/API.md`, `docs/ENGINE.md` | API and engine contracts |
| `website/` | This documentation site (MkDocs Material) |

Start with [Architecture](architecture.md), then [Building from source](building.md),
[Testing](testing.md) and [Contributing](contributing.md).
