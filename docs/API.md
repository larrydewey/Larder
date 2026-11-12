# Larder HTTP API (v1)

All endpoints are under `/api`. Request/response bodies are JSON (`Content-Type: application/json`)
unless noted. Timestamps are RFC 3339 UTC strings with microseconds (e.g. `2026-11-09T12:34:56.123456Z`).
Dates (meal plan) are `YYYY-MM-DD`. IDs are UUID v4 strings. Clients MAY supply their own `id` when
creating recipes, meal plan entries, shopping lists and shopping items (needed for offline creation);
if omitted the server generates one. Creating with an `id` that already exists in the same household
is idempotent: the server returns `200` with the existing resource instead of `201` (so replaying an
offline outbox is safe).

## Errors

Non-2xx responses have the body:

```json
{ "error": "human readable message", "code": "not_found" }
```

Codes: `bad_request`, `unauthorized`, `forbidden`, `not_found`, `conflict`, `unprocessable`,
`llm_disabled`, `internal`.

## Authentication

Bearer JWT access tokens: `Authorization: Bearer <access_token>`. Access tokens live 15 minutes.
Refresh tokens are opaque strings valid for 60 days and are rotated on every refresh.

### `GET /api/config` (public)
```json
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

```json
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

```json
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

```json
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

```json
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
  "status": "cookbook",          // "cookbook" | "trial" (household-wide, see Trial recipes)
  "tried_count": 0,              // how often "I made this" was recorded
  "last_tried_at": null,         // when it was last made, or null
  "created_by": "uuid",
  "created_at": "...",
  "updated_at": "...",
  "version": 3                   // increments on every update
}

RecipeSummary = {
  "id", "slug", "name", "description", "image_url", "tags", "categories", "rating",
  "total_time_minutes", "yield_quantity", "yield_unit", "is_favorite", "status", "tried_count",
  "last_tried_at", "created_at", "updated_at"
}

RecipeInput = Recipe without id(optional), household_id, slug, image_url, is_favorite,
              created_by, created_at, updated_at.
              "version" optional on update: if provided and != current version -> 409 conflict
              (response body includes the current recipe as "current").
              "status" optional: defaults to "cookbook" on create; omitted on update = unchanged.
              "tried_count" / "last_tried_at" are accepted on create only (e.g. moving
              standalone data to a server); updates ignore them.
              Ingredients may be sent with only "original" set; the server parses missing fields.
```

Image URLs are capability URLs (random image id) and do NOT require auth so they work in `<img>` tags.

- `GET /api/recipes?q=&tag=&category=&favorites=true&sort=name|-name|updated|-updated|created|-created|rating|-rating&page=1&per_page=50`
  → `{ "items": [RecipeSummary], "total": 123, "page": 1, "per_page": 50 }`.
  `q` matches name, description, ingredients, tags & categories (all whitespace-separated terms must match).
  `status=cookbook|trial|all`: without it a plain listing returns only cookbook recipes, while a
  search (`q`) covers every recipe (each item carries its `status`). Other values return `400`.
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
- `POST /api/recipes/{id}/tried` → `Recipe` (see Trial recipes)
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

### Trial recipes

A recipe's `status` is `cookbook` (the main collection) or `trial` (recipes the household wants to
try first). Trial recipes are left out of the plain recipe list and of the printed cookbook, but
are found by searches and work everywhere else (meal plan, shopping lists, share links). Viewing a
recipe never changes it; only these explicit actions do:

- Move to the cookbook / back to trial: `PUT /api/recipes/{id}` with `"status"`.
- Discard: `DELETE /api/recipes/{id}`.
- "I made this": `POST /api/recipes/{id}/tried`
```js
{
  "outcome": "keep",                  // "keep": move to the cookbook; "again": status unchanged
  "tried_at": "2026-11-12T18:30:00Z", // optional, default now; also identifies the verdict
  "date": "2026-11-12",               // optional: the cook's local date for the note line
  "rating": 4,                        // optional 1-5; omitted or null keeps the rating
  "note": "too salty, cut soy by half" // optional; appended to notes as "Nov 12: too salty, …"
}
```
  → `Recipe`. Adds 1 to `tried_count`, moves `last_tried_at` forward (never back) and increments
  `version`, in one statement, so verdicts sent from several devices all count. Sending the same
  verdict again (same `tried_at` as the stored `last_tried_at`, e.g. an offline outbox retry)
  changes nothing and returns the recipe. Invalid values return `400`.

### Ingredient parsing
- `POST /api/parse/ingredients` `{ "lines": ["2 cups flour"], "use_llm": false }` → `[Ingredient]`
  (`use_llm: true` requires LLM to be enabled, else `400 llm_disabled`). Returns the raw parse;
  the food-name normalisation applied when recipes are saved is not applied here.

## Offline sync

- `GET /api/sync?since=<cursor>` (omit `since` for a full sync) →
```json
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

```json
MealPlanEntry = {
  "id": "uuid", "date": "2026-11-09",
  "meal": "dinner",              // "breakfast" | "lunch" | "dinner" | "snack"
  "recipe_id": "uuid",           // nullable: free-text entries use "title"
  "recipe_name": "Lasagna",      // read-only convenience (null if no recipe)
  "recipe_image_url": "...",     // read-only, nullable
  "title": null,                 // free text, e.g. "Leftovers"
  "note": null,
  "servings": 4,                 // nullable; used when adding to shopping list
  "recipe_status": "trial",      // read-only: the recipe's status, null without a recipe
  "trial_outcome": null,         // how "Did you make it?" was answered: "keep" | "again" | "discard" | "skipped"
  "trial_resolved_at": null,     // when it was answered
  "created_by": "uuid", "updated_at": "..."
}
```
- `GET /api/mealplan?start=2026-11-09&end=2026-11-15` (inclusive) → `[MealPlanEntry]` sorted by date, meal order.
- `POST /api/mealplan` `{ "id"?, "date", "meal", "recipe_id"?, "title"?, "note"?, "servings"?, "trial_outcome"? }` → `201 MealPlanEntry`
- `PUT /api/mealplan/{id}` same body → `MealPlanEntry`. `trial_outcome` omitted or null leaves the
  answer unchanged, `""` clears it; `trial_resolved_at` is set when the answer changes. Clients ask
  "Did you make it?" once for past entries of trial recipes and record the answer here.
- `DELETE /api/mealplan/{id}` → `204`

## Shopping lists

```json
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

```json
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

Single-recipe imports (`url`, `text`, `image`) save to the trial list unless `status` says
otherwise (previews include the `status` the recipe would get). Bulk imports (`file`, `mealie`)
save to the cookbook, except that Larder archives keep each recipe's stored `status`,
`tried_count` and `last_tried_at`; a `status` field puts every recipe there instead.

- `POST /api/import/url` `{ "url": "https://...", "save": true, "status"?: "trial" }` → `201 Recipe` if `save`, else `200 RecipeInput` (preview).
  URLs pointing at `localhost`, loopback, link-local, unspecified or `0.0.0.0/8` addresses are
  refused (`400`), including after DNS resolution and on every redirect; LAN addresses are allowed.
- `POST /api/import/text` `{ "text": "...", "save": false, "use_llm": false, "status"?: "trial" }` → `RecipeInput` / `201 Recipe`
  (without LLM a heuristic parser looks for Ingredients/Instructions headings)
- `POST /api/import/image` multipart `file` (photo of a recipe), fields `save` = `true|false`, `status` (optional) → requires LLM.
- `POST /api/import/file` multipart: `source` = `auto|mealie|tandoor|paprika|nextcloud|recipekeeper|larder|jsonld`,
  `file` = the export file, `skip_duplicates` = `true|false` (default true),
  `categorize` = `true|false` (default true: recipes without categories get the suggested one),
  `status` = `cookbook|trial` (optional, see above; anything else is `400`) → `202 ImportJob`
  - `mealie`: Mealie recipe export zip or Mealie backup zip
  - `tandoor`: Tandoor export zip (zip of per-recipe zips containing `recipe.json` + image)
  - `paprika`: `.paprikarecipes` file
  - `nextcloud`: zip of the Nextcloud Cookbook folder (`<Recipe>/recipe.json` + `full.jpg`)
  - `recipekeeper`: Recipe Keeper export zip (`recipes.html` + `images/`)
  - `larder`: Larder export zip (`GET /api/export/json`)
  - `jsonld`: a `.json` file containing one schema.org Recipe or an array of them
- `POST /api/import/mealie` `{ "base_url": "https://mealie.example.com", "token": "...", "skip_duplicates": true, "categorize": true, "status"?: "trial" }` → `202 ImportJob`
  (imports via the live Mealie API, including images; `base_url` is subject to the same address
  checks as URL imports)
- `GET /api/import/jobs` → `[ImportJob]` (latest 50 for household)
- `GET /api/import/jobs/{id}` → `ImportJob`

## Export

- `GET /api/export/json` → `application/zip` (`recipes.json` + `images/<recipe_id>.jpg`), re-importable with source `larder`.
  Each recipe carries `status`, `tried_count` and `last_tried_at` (optional on import: older
  archives import into the cookbook).
- `GET /api/export/cookbook/themes` → `[{ "id": "classic", "name": "Classic", "description": "..." }]`
- `POST /api/export/cookbook` →  `application/pdf`
```json
{
  "title": "Family Cookbook",
  "subtitle": "Recipes from our kitchen",   // optional
  "author": "The Smiths",                   // optional
  "dedication": "For Grandma",              // optional
  "theme": "classic",                       // "classic" | "modern" | "rustic"
  "paper": "letter",                        // "letter" | "a4" | "a5"
  "recipe_ids": null,                       // null = all household recipes; trial recipes are always
                                            // left out (400 if nothing printable was picked)
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

Canonical unit keys used in `Ingredient.unit` when recognised (anything else is kept as free text):
volume `tsp`, `tbsp`, `fl oz`, `cup`, `pint`, `quart`, `gallon`, `ml`, `cl`, `dl`, `l`;
mass `oz`, `lb`, `mg`, `g`, `kg`; counts `pinch`, `dash`, `clove`, `can`, `jar`, `package`, `slice`,
`piece`, `stick`, `bunch`, `sprig`, `handful`, `head`, `drop`.
Clients scale `quantity`/`quantity_max` by `target_servings / yield_quantity` and convert between
metric/imperial with the same rules as `larder-core` (`units.rs`): 1 cup = 236.588 ml,
1 tbsp = 14.7868 ml, 1 tsp = 4.92892 ml, 1 fl oz = 29.5735 ml, 1 oz = 28.3495 g, 1 lb = 453.592 g.
The Flutter app's `util/units.dart` is a port of that code.
