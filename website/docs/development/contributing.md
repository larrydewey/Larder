# Contributing

Contributions are welcome: bug reports, import samples, fixes, translations of headings for
the text parser, documentation.

## Reporting bugs

Open an issue on GitHub with:

- the Larder version (Settings → About shows the app and server versions);
- web or Android, standalone or connected, SQLite or PostgreSQL;
- what you did, what you expected and what happened;
- for import problems, if you can, a small anonymised export file that reproduces it.

Server logs help (`docker compose logs larder`, or `RUST_LOG=debug` for more detail). Remove
tokens and personal data before posting.

## Making changes

1. Fork and create a branch.
2. Make the change with tests:
    - parsing, units, aisles, shopping → `server/crates/core` unit tests;
    - an importer → a test in that importer's module using a minimal synthetic export
      (see the existing `make_zip` helpers);
    - an API change → `server/crates/server/tests/api.rs`, and update `docs/API.md` and
      `website/docs/reference/api.md`;
    - an engine method → `docs/ENGINE.md`, the engine tests and the Dart `Engine` interface;
    - app logic → a test under `app/test/`.
3. Run the checks CI runs (see [Testing](testing.md)):

    ```bash
    cd server && cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
    cd ../app && flutter analyze && flutter test
    ```

4. Open a pull request describing the change and how you tested it.

## Guidelines

- **Keep server and phone in step.** Logic that both need belongs in `larder-core` (or
  another shared crate) so standalone and connected mode behave the same. The engine's JSON
  shapes are the API's shapes.
- **Both databases.** SQL must work on SQLite and PostgreSQL (`$N` placeholders, text
  timestamps, integer booleans). Schema changes need a migration in both
  `migrations/sqlite` and `migrations/postgres`.
- **Offline-first.** New mutations in the app go through the outbox with client-generated ids
  and must be idempotent on the server (creating with an existing id returns the existing
  resource).
- **User text is data.** Never interpolate user text into Typst markup, SQL or HTML.
- **No runtime font or CDN fetching** in the app; fonts are bundled.
- Match the existing style: `cargo fmt`, `flutter analyze` clean, small focused functions.

## Documentation

The site is in `website/` and built with MkDocs Material:

```bash
cd website
pip install -r requirements.txt
mkdocs serve
```

Check `mkdocs build --strict` passes before submitting. Screenshots live in
`website/docs/assets/screenshots/` (desktop at 1440 px wide, phone at Pixel 7 size, WebP).

## License

Larder is MIT licensed; by contributing you agree your contributions are licensed the same
way. Bundled fonts are under the SIL Open Font License.
