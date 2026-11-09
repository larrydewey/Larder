# Larder on-device engine (v1)

The Android app can run **standalone** (no server). All logic that normally lives on the server is
then provided by `larder-engine`, a Rust `cdylib` (`liblarder_engine.so`) built from the same
crates as the server (`larder-core`, `larder-import`, `larder-cookbook`) and loaded with `dart:ffi`.

## C ABI

```c
// Calls `method` with a UTF-8 JSON request body. Always returns a UTF-8 JSON response buffer
// (never NULL) whose length is written to *out_len. Free it with larder_free.
uint8_t* larder_call(const char* method /* NUL-terminated */, const uint8_t* input, size_t input_len, size_t* out_len);
void     larder_free(uint8_t* ptr, size_t len);
```

- Successful responses are `{"ok": <result>}`; failures are `{"error": "message", "code": "bad_request|unprocessable|llm_disabled|internal"}`.
- Calls are blocking and thread-safe. Call them from a background isolate (`Isolate.run`) — imports,
  network calls and PDF rendering can take seconds to minutes.
- Files are passed by **path** (the app's cache/documents directories), never as bytes over FFI.
- JSON shapes (`Recipe`, `RecipeInput`, `Ingredient`, `ShoppingItem`, `ImportJob` …) are exactly those in
  [API.md](API.md), so app models are shared between server and engine modes.

## Methods

| Method | Request | `ok` result |
|---|---|---|
| `version` | `{}` | `{ "version": "0.1.0" }` |
| `parse_ingredients` | `{ "lines": ["2 cups flour"] }` | `[Ingredient]` |
| `normalize_recipe` | `RecipeInput` | `RecipeInput` normalised exactly like the server does on save (trims, parses ingredient lines that only have `original`, normalises food names and rewrites understood lines in standard form, computes `total_time_minutes`, dedupes tags, clamps rating) plus `"slug"` and `"search_text"` (lower-cased, accent-folded text for local search) |
| `format_ingredient` | `{ "ingredient": Ingredient, "scale": 1.5, "units": "original\|metric\|imperial" }` | `{ "text": "2 1/4 cups flour" }` |
| `parse_shopping_item` | `{ "text": "2 lbs apples" }` | `{ "food", "quantity", "unit", "note", "aisle", "display" }` |
| `classify_aisle` | `{ "food": "milk" }` | `{ "aisle": "Dairy & Eggs" }` |
| `merge_shopping` | `{ "items": [ShoppingItem], "add": [{ "recipe": Recipe, "servings": 6 \| null }] }` | `{ "items": [ShoppingItem] }` — the complete new item list (existing items updated in place, new items get fresh UUIDs, `aisle` + `display` filled), using the same merge rules as the server |
| `suggest_categories` | `{ "recipes": [RecipeInput & { "id" }] }` | `{ "categories": ["Breakfast", …], "suggestions": [{ "id", "category": "Soups & Stews" \| null }] }` — Larder's category list (in meal order) and the suggested category for each recipe, by the same rules as `POST /api/recipes/organize` |
| `process_image` | `{ "input_path": "...", "output_dir": "...", "id": "uuid" }` | `{ "thumb": path, "medium": path, "full": path }` — JPEGs (320/960/2400 px) |
| `import_file` | `{ "source": "auto\|mealie\|tandoor\|paprika\|nextcloud\|recipekeeper\|larder\|jsonld", "path": "...", "filename": "x.zip", "image_dir": "..." }` | `{ "source": "mealie", "recipes": [ImportedRecipe], "errors": ["..."] }` |
| `import_url` | `{ "url": "https://...", "image_dir": "..." }` | `ImportedRecipe` |
| `import_text` | `{ "text": "..." }` | `RecipeInput` (heuristic parser) |
| `mealie_list` | `{ "base_url", "token" }` | `{ "slugs": ["..."] }` |
| `mealie_fetch` | `{ "base_url", "token", "slug", "image_dir" }` | `ImportedRecipe` |
| `llm_recipe_from_text` | `{ "llm": LlmConfig, "text": "..." }` | `RecipeInput` |
| `llm_recipe_from_image` | `{ "llm": LlmConfig, "image_path": "..." }` | `RecipeInput` |
| `llm_parse_ingredients` | `{ "llm": LlmConfig, "lines": [...] }` | `[Ingredient]` |
| `cookbook_themes` | `{}` | `[{ "id", "name", "description" }]` |
| `render_cookbook` | `{ "options": CookbookOptions (API.md, without recipe_ids), "recipes": [{ "id", "data": RecipeInput, "image_path": "..." \| null }], "out_path": ".../book.pdf" }` | `{ "path": out_path, "bytes": 123456 }` |
| `export_backup` | `{ "recipes": [{ "id", "data": RecipeInput, "image_path", "favorite", "created_at", "updated_at" }], "out_path": ".../larder-backup.zip" }` | `{ "path": out_path }` — same zip format as the server's `GET /api/export/json`, importable with source `larder` on either side |

```json
ImportedRecipe = {
  "data": RecipeInput,
  "image_path": "/…/image_dir/<uuid>.jpg" | null,   // downloaded/extracted image, already a JPEG
  "favorite": false,
  "created_at": "..." | null
}
LlmConfig = { "provider": "anthropic" | "openai" | "ollama", "api_key": "..." | null, "model": "..." | null, "base_url": "..." | null }
```

## Building

`app/tool/build_engine.sh` cross-compiles the library with `cargo-ndk` for `arm64-v8a`,
`armeabi-v7a` and `x86_64` into `app/android/app/src/main/jniLibs/`. CI runs it before
`flutter build apk`. For `flutter test` on Linux the script's `--host` flag builds
`server/target/release/liblarder_engine.so`, which tests load via `LARDER_ENGINE_LIB`.
