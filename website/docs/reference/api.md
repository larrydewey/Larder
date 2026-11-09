# HTTP API

This is the complete reference for the Larder server's JSON API (version 1), the same API the
app uses. Everything the app can do, you can script.

!!! tip "Quick start"

    ```bash
    BASE=https://recipes.example.com
    TOKEN=$(curl -s $BASE/api/auth/login -H 'Content-Type: application/json' \
      -d '{"username":"alice","password":"secret123"}' | jq -r .access_token)
    curl -s "$BASE/api/recipes?q=pancakes" -H "Authorization: Bearer $TOKEN" | jq
    ```

    Access tokens expire after 15 minutes; use `POST /api/auth/refresh` with the refresh token
    to get a new pair.

Unless marked **public**, every endpoint needs `Authorization: Bearer <access_token>`. A
missing or invalid token gives `401 unauthorized`; endpoints for household owners or server
admins give `403 forbidden` to others. Request bodies larger than `LARDER_MAX_UPLOAD_MB` are
rejected. Unknown `/api/...` paths return `404` with `{"error": "no such API endpoint"}`.
`GET /healthz` (outside `/api`, public) returns `200 ok` for health checks.


All endpoints are under `/api`. Request/response bodies are JSON (`Content-Type: application/json`)
unless noted. Timestamps are RFC 3339 UTC strings with microseconds (e.g. `2026-11-09T12:34:56.123456Z`).
Dates (meal plan) are `YYYY-MM-DD`. IDs are UUID v4 strings. Clients MAY supply their own `id` when
creating recipes, meal plan entries, shopping lists and shopping items (needed for offline creation);
if omitted the server generates one. Creating with an `id` that already exists in the same household
is idempotent: the server returns `200` with the existing resource instead of `201` (so replaying an
offline outbox is safe).

## Errors

Non-2xx responses have the body:

```js
{ "error": "human readable message", "code": "not_found" }
```

Codes: `bad_request`, `unauthorized`, `forbidden`, `not_found`, `conflict`, `unprocessable`,
`llm_disabled`, `internal`.

## Authentication

Bearer JWT access tokens: `Authorization: Bearer <access_token>`. Access tokens live 15 minutes.
Refresh tokens are opaque strings valid for 60 days and are rotated on every refresh.

### `GET /api/config` (public)
```js
{
  "version": "0.1.0",
  "setup_required": false,          // true when no users exist: first registration becomes admin
  "registration_open": false,       // true: anyone may register (creates their own household)
  "public_sharing": true,           // members may create public share links (LARDER_PUBLIC_SHARING)
  "oidc": { "enabled": true, "name": "Authentik" },
  "llm_enabled": true,
  "database": "sqlite"              // or "postgres"
}
```

### `POST /api/auth/register` (public)
Body: `{ "username": "alice", "password": "min 8 chars", "display_name": "Alice", "email": "a@b.c", "invite_code": "optional" }`

- If no users exist: user becomes server admin and owner of a new household.
- If `invite_code` is valid: user joins that household as `member`.
- Else if registration is open: user gets a new household as `owner`.
- Else `403 forbidden`.

Response `201`: `AuthResponse`.

### `POST /api/auth/login` (public)
Body: `{ "username": "alice", "password": "..." }` (username is case-insensitive; email also accepted)
Response `200`: `AuthResponse`

```js
AuthResponse = {
  "access_token": "jwt",
  "refresh_token": "opaque",
  "expires_in": 900,
  "user": User
}
```

### `POST /api/auth/refresh` (public)
Body: `{ "refresh_token": "..." }` → `AuthResponse` (new refresh token; old one is revoked).

### `POST /api/auth/logout`
Body: `{ "refresh_token": "..." }` → `204`.

### OIDC
- `GET /api/auth/oidc/login?redirect_uri=<app url>` → 302 to the identity provider.
  `redirect_uri` must be either a path/URL on the server's own `PUBLIC_URL` or the app scheme
  `larder://auth`.
- `GET /api/auth/oidc/callback` (provider redirects here) → 302 to
  `<redirect_uri>#access_token=...&refresh_token=...&expires_in=900`
  or `<redirect_uri>#error=<message>` on failure.
  New OIDC users are created automatically (joining a new household, or the household of a pending
  invite code passed as `invite_code` query param on the login URL).

## Users

```js
User = {
  "id": "uuid",
  "username": "alice",
  "display_name": "Alice",
  "email": "a@b.c",                  // may be null
  "is_admin": true,
  "household_id": "uuid",
  "household_role": "owner",         // "owner" | "member"
  "preferences": { "units": "original" },   // "original" | "metric" | "imperial"
  "created_at": "..."
}
```

- `GET /api/users/me` → `User`
- `PATCH /api/users/me` body (all optional): `{ "display_name", "email", "preferences", "current_password", "new_password", "refresh_token" }` → `User`
  (changing password requires `current_password` unless the user has no password, i.e. OIDC-only).
  `email`: omitted = unchanged, `null` or `""` = remove, a string = set.
  Changing the password revokes all of the user's refresh tokens except `refresh_token`, if the
  caller sends its own, so the calling device stays signed in.

## Households

```js
Household = {
  "id": "uuid", "name": "Smith Family", "created_at": "...",
  "members": [ { "id": "uuid", "username": "alice", "display_name": "Alice", "role": "owner" } ]
}
Invite = { "code": "8-char code", "created_at": "...", "expires_at": "...", "created_by": "uuid" }
```

- `GET /api/household` → `Household`
- `PATCH /api/household` `{ "name": "..." }` (owner) → `Household`
- `GET /api/household/invites` (owner) → `[Invite]`
- `POST /api/household/invites` (owner) `{ "expires_in_days": 7 }` → `201 Invite`
- `DELETE /api/household/invites/{code}` (owner) → `204`
- `PATCH /api/household/members/{user_id}` (owner) `{ "role": "owner" | "member" }` → `Household`
- `DELETE /api/household/members/{user_id}` (owner; cannot remove self) → `204`. The removed user is
  moved to a new empty household of their own.

## Admin (server admins only)

- `GET /api/admin/users` → `[User & { "disabled": bool }]`
- `PATCH /api/admin/users/{id}` `{ "is_admin"?: bool, "disabled"?: bool, "new_password"?: string }` → `User`
- `DELETE /api/admin/users/{id}` → `204`
- `GET /api/admin/settings/ai` → `AiSettings`
- `PUT /api/admin/settings/ai` `{ "provider": "anthropic"|"openai"|"ollama", "api_key"?: string, "model"?: string, "base_url"?: string }` → `AiSettings`.
  Applies immediately. Omit `api_key` to keep the saved key; it is only kept for the same provider and
  base URL. `400` for invalid values or a missing key, `409` when `LARDER_LLM_*` environment variables
  configure the assistant.
- `DELETE /api/admin/settings/ai` → `204` (turns AI assist off; `409` with environment variables)
- `POST /api/admin/settings/ai/test` → `{ "ok": true, "model": "…" }` after one small request, or `422`
  with the provider's error

  `AiSettings = { "source": "environment"|"server"|"off", "enabled": bool, "provider", "model", "base_url",
  "has_key": bool, "key_hint": "last 4 characters" | null, "key_unreadable": bool,
  "defaults": { "<provider>": { "model", "base_url", "needs_key" } } }`. The key itself is never returned.

## Recipes

Recipes belong to a household; every member can read and edit them.

```js
Ingredient = {
  "section": null,          // optional group heading, e.g. "For the sauce"
  "quantity": 1.5,          // null when not a measured ingredient ("salt to taste")
  "quantity_max": null,     // for ranges: "2-3 cloves" => quantity 2, quantity_max 3
  "unit": "cup",            // canonical unit key or free text, null if none (see Units)
  "food": "flour",          // may be null when unparsed
  "note": "sifted",         // may be null
  "original": "1 1/2 cups flour, sifted"   // always present: the text as entered
}
Instruction = { "section": null, "text": "Preheat the oven..." }

Recipe = {
  "id": "uuid",
  "household_id": "uuid",
  "slug": "classic-pancakes",
  "name": "Classic Pancakes",
  "description": "",
  "yield_quantity": 4,           // number or null
  "yield_unit": "servings",      // free text, default "servings"
  "prep_time_minutes": 10,       // nullable ints
  "cook_time_minutes": 15,
  "total_time_minutes": 25,
  "source_url": null,
  "source_name": null,           // e.g. "Grandma", "NYT Cooking", "Imported from Mealie"
  "notes": "",
  "rating": 4,                   // 0..5 or null
  "ingredients": [Ingredient],
  "instructions": [Instruction],
  "tags": ["breakfast", "quick"],
  "categories": ["Breakfast"],   // used as cookbook chapters
  "nutrition": { "calories": "250 kcal" },  // string -> string
  "image_url": "/api/images/<image_id>",    // null when no image; append ?size=thumb|medium|full (default medium)
  "is_favorite": false,          // for the requesting user
  "created_by": "uuid",
  "created_at": "...",
  "updated_at": "...",
  "version": 3                   // increments on every update
}

RecipeSummary = {
  "id", "slug", "name", "description", "image_url", "tags", "categories", "rating",
  "total_time_minutes", "yield_quantity", "yield_unit", "is_favorite", "updated_at"
}

RecipeInput = Recipe without id(optional), household_id, slug, image_url, is_favorite,
              created_by, created_at, updated_at.
              "version" optional on update: if provided and != current version -> 409 conflict
              (response body includes the current recipe as "current").
              Ingredients may be sent with only "original" set; the server parses missing fields.
```

Image URLs are capability URLs (random image id) and do NOT require auth so they work in `<img>` tags.

- `GET /api/recipes?q=&tag=&category=&favorites=true&sort=name|-name|updated|-updated|created|-created|rating|-rating&page=1&per_page=50`
  → `{ "items": [RecipeSummary], "total": 123, "page": 1, "per_page": 50 }`.
  `q` matches name, description, ingredients, tags & categories (all whitespace-separated terms must match).
  `tag` and `category` may be repeated (all must match). Default sort `name`. `per_page` max 500.
- `POST /api/recipes` `RecipeInput` → `201 Recipe`
- `GET /api/recipes/{id_or_slug}` → `Recipe`
- `PUT /api/recipes/{id}` `RecipeInput` → `Recipe`
- `DELETE /api/recipes/{id}` → `204`
- `POST /api/recipes/{id}/duplicate` → `201 Recipe`
- `GET /api/recipes/{id}/share` → `200 RecipeShare`, or `404` if the recipe has no public link
- `POST /api/recipes/{id}/share` → `201 RecipeShare` (new) or `200 RecipeShare` (the existing link);
  `403` when `public_sharing` is off
- `DELETE /api/recipes/{id}/share` → `204`; the link stops working

  `RecipeShare = { "token": "32 hex chars", "url": "<LARDER_PUBLIC_URL>/s/<token>", "created_at": "…" }`.
  The public page `GET /s/{token}` (outside `/api`, no authentication) returns HTML: the recipe,
  or `404` "Recipe not available".
- `PUT /api/recipes/{id}/favorite` → `204`, `DELETE /api/recipes/{id}/favorite` → `204`
- `PUT /api/recipes/{id}/image` multipart form field `file` (jpeg/png/webp/gif) → `Recipe`
- `DELETE /api/recipes/{id}/image` → `Recipe`
- `GET /api/images/{image_id}?size=thumb|medium|full` (public) → image/jpeg (thumb 320px, medium 960px, full original-ish ≤ 2400px)
- `GET /api/tags` → `[{ "name": "quick", "count": 12 }]`
- `GET /api/categories` → `[{ "name": "Breakfast", "count": 7 }]`
- `POST /api/tags/rename` `{ "kind": "tag"|"category", "from": "a", "to": "b" }` → `204` (merge if `to` exists)
- `POST /api/tags/delete` `{ "kind": "tag"|"category", "name": "a" }` → `204`

### Organizing recipes
- `POST /api/recipes/organize` → tidies the household's recipes and fills in categories.
```js
// request (all optional)
{
  "categorize": "uncategorized",   // "uncategorized" (default): only recipes without categories
                                   // "all": put the suggestion first on every recipe that has one
                                   // "none": leave categories alone
  "normalize": true,               // re-run ingredient normalisation (default true)
  "dry_run": false                 // true: report what would change, save nothing
}
// response
{
  "dry_run": false,
  "recipes_changed": 12,
  "categorized": 9,                // recipes whose first category changed
  "normalized": 5,                 // recipes whose ingredients changed
  "changes": [
    { "id": "uuid", "name": "Classic Chili", "category_before": null,
      "category_after": "Soups & Stews", "ingredients_changed": true }
  ]
}
```
  Suggested categories are one of `Breakfast`, `Appetizers & Snacks`, `Soups & Stews`, `Salads`,
  `Main Dishes`, `Side Dishes`, `Breads`, `Desserts`, `Drinks`, `Sauces & Condiments`,
  `Seasonings & Spice Mixes`, `Household & DIY`. With `"all"` the suggestion is inserted first and
  any other categories are kept after it. Changed recipes are saved like a normal update (their
  `version` increments). An invalid `categorize` value returns `400`.

### Ingredient parsing
- `POST /api/parse/ingredients` `{ "lines": ["2 cups flour"], "use_llm": false }` → `[Ingredient]`
  (`use_llm: true` requires LLM to be enabled, else `400 llm_disabled`). Returns the raw parse;
  the food-name normalisation applied when recipes are saved is not applied here.

## Offline sync

- `GET /api/sync?since=<cursor>` (omit `since` for a full sync) →
```js
{
  "cursor": "opaque string; pass as since next time",
  "full": true,                         // true if this is a full snapshot (client should replace local data)
  "recipes": [Recipe],                  // created/updated since cursor
  "deleted_recipe_ids": ["uuid"],       // deleted since cursor (empty when full)
  "meal_plan": [MealPlanEntry],         // always the full window (UTC) today-31d .. today+61d
  "shopping_lists": [ShoppingList]      // always complete, with items
}
```
Clients keep an outbox of mutating requests while offline and replay them in order once online.
Conflicts (`409`) on replay are resolved by the client re-fetching (server wins) and surfacing a
notice. Creates with client-generated IDs are idempotent (see top).

## Meal planner

```js
MealPlanEntry = {
  "id": "uuid", "date": "2026-11-09",
  "meal": "dinner",              // "breakfast" | "lunch" | "dinner" | "snack"
  "recipe_id": "uuid",           // nullable: free-text entries use "title"
  "recipe_name": "Lasagna",      // read-only convenience (null if no recipe)
  "recipe_image_url": "...",     // read-only, nullable
  "title": null,                 // free text, e.g. "Leftovers"
  "note": null,
  "servings": 4,                 // nullable; used when adding to shopping list
  "created_by": "uuid", "updated_at": "..."
}
```
- `GET /api/mealplan?start=2026-11-09&end=2026-11-15` (inclusive) → `[MealPlanEntry]` sorted by date, meal order.
- `POST /api/mealplan` `{ "id"?, "date", "meal", "recipe_id"?, "title"?, "note"?, "servings"? }` → `201 MealPlanEntry`
- `PUT /api/mealplan/{id}` same body → `MealPlanEntry`
- `DELETE /api/mealplan/{id}` → `204`

## Shopping lists

```js
ShoppingItem = {
  "id": "uuid", "list_id": "uuid",
  "food": "flour",               // nullable
  "quantity": 2.5,               // nullable
  "unit": "cup",                 // nullable
  "note": null,
  "display": "2 1/2 cups flour", // read-only, server formatted
  "aisle": "Baking",             // auto-assigned when omitted; editable
  "checked": false,
  "position": 0,
  "recipe_ids": ["uuid"],        // recipes this item came from
  "updated_at": "..."
}
ShoppingList = {
  "id": "uuid", "name": "Weekly", "created_at": "...", "updated_at": "...",
  "item_count": 10, "checked_count": 3,
  "items": [ShoppingItem]        // only on GET /api/shopping-lists/{id} and in sync
}
```
Aisles (fixed ordering used for grouping): `Produce`, `Meat & Seafood`, `Dairy & Eggs`, `Bakery`,
`Pantry`, `Baking`, `Spices & Seasonings`, `Canned & Jarred`, `Frozen`, `Beverages`, `Household`, `Other`.

- `GET /api/shopping-lists` → `[ShoppingList]` (without items)
- `POST /api/shopping-lists` `{ "id"?, "name" }` → `201 ShoppingList`
- `GET /api/shopping-lists/{id}` → `ShoppingList` with items
- `PATCH /api/shopping-lists/{id}` `{ "name" }` → `ShoppingList`
- `DELETE /api/shopping-lists/{id}` → `204`
- `POST /api/shopping-lists/{id}/items` `{ "id"?, "text"? , "food"?, "quantity"?, "unit"?, "note"?, "aisle"? }`
  (if `text` is given it is parsed like an ingredient line) → `201 ShoppingItem`
- `PATCH /api/shopping-lists/{id}/items/{item_id}` any of `{ "food", "quantity", "unit", "note", "aisle", "checked", "position" }` → `ShoppingItem`
- `DELETE /api/shopping-lists/{id}/items/{item_id}` → `204`
- `POST /api/shopping-lists/{id}/clear-checked` → `ShoppingList` with items
- `POST /api/shopping-lists/{id}/add-recipes` `{ "recipes": [{ "recipe_id": "uuid", "servings": 6 }] }` → `ShoppingList` with items.
  Ingredients are scaled to `servings` (when recipe has a yield), merged with existing unchecked items
  having the same food and compatible unit.
- `POST /api/shopping-lists/{id}/add-mealplan` `{ "start": "2026-11-09", "end": "2026-11-15" }` → `ShoppingList` with items

## Import

```js
ImportJob = {
  "id": "uuid", "source": "mealie", "status": "pending|running|done|failed",
  "total": 120, "imported": 118, "skipped": 1, "failed": 1,
  "errors": ["Recipe 'X': missing name"],
  "recipe_ids": ["uuid"],
  "created_at": "...", "finished_at": null
}
```
Imported recipes are normalised like any saved recipe. Single-recipe imports (`url`, `text`, and
`image` previews) give a recipe without categories the suggested category (see
`POST /api/recipes/organize`).

- `POST /api/import/url` `{ "url": "https://...", "save": true }` → `201 Recipe` if `save`, else `200 RecipeInput` (preview).
  URLs pointing at `localhost`, loopback, link-local, unspecified or `0.0.0.0/8` addresses are
  refused (`400`), including after DNS resolution and on every redirect; LAN addresses are allowed.
- `POST /api/import/text` `{ "text": "...", "save": false, "use_llm": false }` → `RecipeInput` / `201 Recipe`
  (without LLM a heuristic parser looks for Ingredients/Instructions headings)
- `POST /api/import/image` multipart `file` (photo of a recipe), field `save` = `true|false` → requires LLM.
- `POST /api/import/file` multipart: `source` = `auto|mealie|tandoor|paprika|nextcloud|recipekeeper|larder|jsonld`,
  `file` = the export file, `skip_duplicates` = `true|false` (default true),
  `categorize` = `true|false` (default true: recipes without categories get the suggested one) → `202 ImportJob`
  - `mealie`: Mealie recipe export zip or Mealie backup zip
  - `tandoor`: Tandoor export zip (zip of per-recipe zips containing `recipe.json` + image)
  - `paprika`: `.paprikarecipes` file
  - `nextcloud`: zip of the Nextcloud Cookbook folder (`<Recipe>/recipe.json` + `full.jpg`)
  - `recipekeeper`: Recipe Keeper export zip (`recipes.html` + `images/`)
  - `larder`: Larder export zip (`GET /api/export/json`)
  - `jsonld`: a `.json` file containing one schema.org Recipe or an array of them
- `POST /api/import/mealie` `{ "base_url": "https://mealie.example.com", "token": "...", "skip_duplicates": true, "categorize": true }` → `202 ImportJob`
  (imports via the live Mealie API, including images; `base_url` is subject to the same address
  checks as URL imports)
- `GET /api/import/jobs` → `[ImportJob]` (latest 50 for household)
- `GET /api/import/jobs/{id}` → `ImportJob`

## Export

- `GET /api/export/json` → `application/zip` (`recipes.json` + `images/<recipe_id>.jpg`), re-importable with source `larder`.
- `GET /api/export/cookbook/themes` → `[{ "id": "classic", "name": "Classic", "description": "..." }]`
- `POST /api/export/cookbook` →  `application/pdf`
```js
{
  "title": "Family Cookbook",
  "subtitle": "Recipes from our kitchen",   // optional
  "author": "The Smiths",                   // optional
  "dedication": "For Grandma",              // optional
  "theme": "classic",                       // "classic" | "modern" | "rustic"
  "paper": "letter",                        // "letter" | "a4" | "a5"
  "recipe_ids": null,                       // null = all household recipes
  "group_by": "category",                   // "category" | "tag" | "none"
  "chapter_order": ["Breakfast", "Mains"],  // optional explicit chapter ordering
  "include_images": true,
  "include_toc": true,
  "include_index": true,
  "include_notes": true,
  "include_nutrition": false,
  "units": "original"                       // "original" | "metric" | "imperial"
}
```

## Units

Canonical unit keys used in `Ingredient.unit` when recognised (anything else is kept as free
text):

- volume: `tsp`, `tbsp`, `fl oz`, `cup`, `pint`, `quart`, `gallon`, `ml`, `cl`, `dl`, `l`;
- mass: `oz`, `lb`, `mg`, `g`, `kg`;
- counts: `pinch`, `dash`, `clove`, `can`, `jar`, `package`, `slice`, `piece`, `stick`,
  `bunch`, `sprig`, `handful`, `head`, `drop`.

Every alias the parser accepts is listed in
[Ingredient parsing & units](ingredients.md#unit-aliases).
Clients scale `quantity`/`quantity_max` by `target_servings / yield_quantity` and convert
between metric/imperial with the same rules as `larder-core` (`units.rs`; the Flutter app's
`util/units.dart` is a port): 1 cup = 236.588 ml, 1 tbsp = 14.7868 ml, 1 tsp = 4.92892 ml,
1 fl oz = 29.5735 ml, 1 oz = 28.3495 g, 1 lb = 453.592 g. See
[Conversion](ingredients.md#conversion).

## Behaviour notes

These details are not obvious from the schemas above:

- **Validation.** Usernames: 2–64 characters of letters, digits, `_ - . @`. Passwords: at
  least 8 characters. Email: must contain `@`, at most 254 characters. Display, household and
  list names: 1–100 characters. Recipe names: required, at most 300 characters. Meal plan
  `meal` must be one of the four values and `date` must be `YYYY-MM-DD`; an entry needs a
  `recipe_id` or a non-empty `title`. Client-supplied `id`s must be UUIDs.
- **Recipe normalisation** on every create and update: strings are trimmed and whitespace
  collapsed; `yield_unit` defaults to `servings`; non-positive yields and times become `null`;
  `total_time_minutes` is computed as prep + cook when missing; `rating` is clamped to 0–5 and
  0 becomes `null`; empty ingredients, steps and nutrition entries are removed; tags and
  categories are de-duplicated case-insensitively and entries over 80 characters dropped;
  units are mapped to canonical keys; ingredients that only have `original` are parsed; then
  every ingredient's food name is [normalised](ingredients.md#normalisation) and, if the line
  was understood, `original` is rewritten in standard form.
- **Slugs** are made from the name (lower case, accents folded, non-alphanumerics replaced by
  `-`, at most 80 characters, `recipe` if empty) and made unique per household with `-2`,
  `-3`… The slug only changes when the name changes.
- **Conflicts.** `PUT /api/recipes/{id}` with a stale `version` returns `409` with
  `{"error": "the recipe was changed by someone else", "code": "conflict", "current": Recipe}`.
  Omitting `version` overwrites unconditionally. Favouriting does not change `version`
  (only `updated_at`, so that sync picks it up).
- **Images.** `PUT` and `POST /api/recipes/{id}/image` are equivalent. The upload field is
  `file`; if it is missing, the first file field is used. Images over 80 megapixels or in
  other formats are rejected with `400`. `GET /api/images/{id}` also accepts `{id}.jpg` and
  the sizes `small` (= thumb), `original`/`large` (= full); anything else gives `medium`.
- **Login** returns `401` for wrong credentials and `403` ("this account is disabled") for a
  disabled account. `POST /api/auth/logout` requires the access token as well.
- **Register** with an invalid or expired invite code returns `400` ("invalid or expired
  invite code"); a taken username or email returns `409`.
- **Changing a password** with `PATCH /api/users/me` revokes all of the user's refresh tokens
  except the `refresh_token` sent in the same request (if any). An admin `new_password` or
  disabling a user revokes all of them. Admins cannot demote, disable or
  delete themselves (`400`).
- **Invites** expire after `expires_in_days` days, clamped to 1–365 (default 7, also when the
  body is empty or not JSON). `GET /api/household/invites` only lists unexpired invites. Codes
  are case-insensitive.
- **Sync.** `since` is the previous `cursor`; the server subtracts 10 seconds to cover
  in-flight writes, so clients must upsert. An unparsable `since` is treated as a full sync.
  The meal plan window is computed on the server from (UTC) today − 31 days to today + 61 days.
- **Shopping items.** `POST …/items` needs `text`, `food` or `note`. A given `aisle` that
  matches a standard aisle case-insensitively is normalised to its spelling; other values are
  kept as custom aisles (sorted together with `Other`). Items are returned in aisle order, then
  `position`, then display text. `PATCH` with `"aisle": ""` re-classifies the item.
  `add-mealplan` adds every entry with a recipe in the date range, scaled to the entry's
  `servings`.
- **Imports.** `POST /api/import/url` defaults to `save: true`; `/api/import/text` defaults
  to `save: false` and limits text to 200,000 bytes; `/api/import/image` requires AI assist.
  Unparsable content returns `422 unprocessable`. URLs (and Mealie `base_url`s) pointing at
  `localhost`, loopback, link-local, unspecified or `0.0.0.0/8` addresses (also as
  IPv4-mapped IPv6) are refused with `400`; DNS results and every redirect are checked too, and
  image URLs found in imports are skipped if they point at such addresses.
  `categorize` (default `true`) gives recipes without categories the suggested category in
  file and Mealie jobs; URL, text and image previews always get it. `skip_duplicates` (default `true`) skips a
  recipe whose `source_url` or accent-folded name already exists in the household.
  `POST /api/import/mealie` validates the token by listing recipes before returning `202`;
  `ImportJob.source` is the detected format when `auto` was requested. `errors` keeps at most
  200 messages. Jobs left `pending`/`running` by a restart become `failed`.
- **Exports.** `GET /api/export/json` responds with
  `Content-Disposition: attachment; filename="larder-export-YYYY-MM-DD.zip"` and also writes
  `manifest.json`. `POST /api/export/cookbook` responds with a PDF named after the slugified
  title. Unknown `theme` falls back to `classic` and unknown `paper` to `letter`; any option
  may be omitted (defaults: title "Our Family Cookbook", `classic`, `letter`, `category`,
  images/TOC/index/notes on, nutrition off, `original` units). `recipe_ids` may contain ids or
  slugs. More than 5000 recipes returns `400`.
- **AI.** Endpoints that need an LLM return `400` with code `llm_disabled` when it is not
  configured; provider failures return `422`.
- **Rate limiting** is not built in.
