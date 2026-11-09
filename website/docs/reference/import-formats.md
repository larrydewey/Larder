# Import formats

This page lists, for each supported format, how Larder recognises it and which source fields
end up in which Larder fields. The same importers run on the server and in the on-device
engine. For a user-level walkthrough see [Importing recipes](../guide/importing.md).

## Common rules

These apply to every format:

- **Normalisation.** Every imported recipe goes through the same normalisation as a recipe
  saved in the editor (see [HTTP API → Behaviour notes](api.md#behaviour-notes)): ingredient
  lines are parsed and [tidied](ingredients.md#normalisation), tags de-duplicated, total time
  computed from prep + cook when missing.
- **Categories.** A recipe without categories gets the
  [suggested category](../guide/organize.md#how-a-category-is-chosen), unless the import was
  started with `categorize=false` (file and Mealie imports only).
- **HTML in fields** is stripped (block tags become line breaks) and HTML entities are decoded,
  including double-encoded ones (`&amp;amp;`).
- **Source name.** If a recipe has no source name, it is set to the host of its source URL
  without `www.` (for example `allrecipes.com`), or else the format's name ("Mealie",
  "Tandoor", "Paprika", "Nextcloud Cookbook", "Recipe Keeper", "JSON-LD"). Larder backups are
  left as they are.
- **Recipes without a name** are rejected: "Recipe '…': recipe name is required".
- **Durations** may be ISO 8601 (`PT1H20M`, `P0DT1H`, `PT0.5H`), clock times (`1:30`), plain
  minutes (`90`) or text (`1 hour 20 mins`, `1h20m`, `1 1/2 hours`, `an hour`, `10-15 minutes`
  → 10, and German, French, Spanish and Italian words such as `2 Stunden`). Zero or
  unparseable durations are dropped.
- **Yields** such as `4`, `Serves 4`, `Makes 12 cookies`, `4-6 servings` (→ 4), `Yield: 1
  loaf`, `one 9-inch pie`, `4 Portionen`, `Servings 6` give a quantity and unit; units meaning
  people or portions become `servings`. Unparseable yields are dropped.
- **Dates** are converted to UTC; dates before 1980 (exporters' "unknown") are ignored.
- **Images.** Embedded images are kept if they are JPEG, PNG, WebP, GIF, AVIF, HEIC or BMP by
  their magic bytes; image URLs are downloaded (at most 20 MB) unless they point at a local or
  link-local address (see [Security](../admin/security.md#importing-from-urls)). The server stores JPEG, PNG,
  WebP and GIF; others are dropped and the recipe imported without a photo.
- **Ratings** are rounded to whole stars and clamped to 0–5.
- **Nutrition keys** are normalised to schema.org names (`calories`, `fatContent`,
  `saturatedFatContent`, `transFatContent`, `unsaturatedFatContent`, `carbohydrateContent`,
  `sugarContent`, `fiberContent`, `proteinContent`, `sodiumContent`, `cholesterolContent`,
  `servingSize`) from variants like `fat_content`, `TotalFat`, `carbs`, `fibre`.

## Detection

With **Detect automatically** (`source=auto`):

| File | Detected as |
|---|---|
| Zip containing `database.json` | Mealie backup |
| Zip with `manifest.json` mentioning `larder-export` / `recipers-export`, or a top-level `recipes.json` | Larder |
| Zip with `recipes.html` containing `recipe-details` | Recipe Keeper (another app's `recipes.html` is refused) |
| Zip with `.paprikarecipe` files | Paprika |
| Zip containing `.zip` files | Tandoor |
| Zip with `<slug>/<slug>.json` | Mealie recipe export |
| Zip with `recipe.json` that has `steps` and no `@type` | Tandoor (single recipe) |
| Zip with `recipe.json` otherwise | Nextcloud Cookbook |
| Zip with other `.json` files | schema.org JSON-LD |
| Gzip file | Paprika (single `.paprikarecipe`) |
| JSON with `"format": "larder-export"`, or objects with `id` and `yield_unit` | Larder |
| JSON containing a schema.org `Recipe` | JSON-LD |
| JSON objects with `slug` and `recipe_ingredient`/`recipeIngredient` | Mealie |
| JSON objects with `steps` and `working_time` or `keywords` | Tandoor |
| JSON objects with `directions` and string `ingredients` | Paprika |
| HTML containing `recipe-details` | Recipe Keeper `recipes.html` |
| Other HTML | A saved web page (JSON-LD / microdata) |

## Mealie

**Recipe export zip**: `recipes/<slug>/<slug>.json` with images in
`recipes/<slug>/images/`, preferring `original.webp`, then `.jpg`, `.jpeg`, `.png`,
`min-original.webp`, then any file containing `original` (not `tiny-…`).

**Backup zip**: `database.json` (the tables `recipes`, `recipes_ingredients`,
`recipe_instructions`, `ingredient_units`, `ingredient_foods`, `tags`, `categories`,
`recipes_to_tags`, `recipes_to_categories`, `recipe_nutrition`, `recipe_settings`, `notes`,
`users_to_recipes`, `users_to_favorites`) and images under `…/recipes/<id>/images/`.

**Live API**: `GET /api/recipes?page=…&perPage=50&orderBy=created_at` to list slugs, then
`GET /api/recipes/<slug>` and `GET /api/media/recipes/<id>/images/original.webp`, with the
API token as a bearer token.

Both snake_case (exports, backups) and camelCase (API) field names are accepted.

| Mealie | Larder |
|---|---|
| `name` | name |
| `description` | description |
| `recipe_servings` (if > 0) | yield (unit "servings") |
| else `recipe_yield_quantity` + `recipe_yield` | yield quantity and unit (unit parsed from the text) |
| else `recipe_yield` text | yield parsed from text |
| `prep_time` | prep time |
| `perform_time` / `cook_time` | cook time |
| `total_time` | total time |
| `org_url` | source URL (http/https only) |
| `rating` (backups: the best per-user rating if the recipe has none) | rating |
| `recipe_ingredient[]` | ingredients; `title` starts a new section; structured `quantity`, `unit` (name, or abbreviation when only that is a known unit), `food`, `note` are kept; unparsed lines use `note` (Mealie stores the whole line there with a default quantity of 0 or 1, which is ignored; any other quantity is put in front unless the note already starts with a number), else `original_text`, else `display`; `disable_amount` (per recipe or ingredient) imports lines as text |
| `recipe_instructions[]` | steps; `title` starts a new section |
| `tags[]`, `recipeCuisine` | tags |
| `recipe_category[]` | categories |
| `nutrition` | nutrition |
| `notes[]` (`title` + `text`) and `extras` (as `key: value` lines) | notes |
| `created_at` / `date_added` | creation date |
| backups: `users_to_favorites`, `users_to_recipes.is_favorite` | favourite (for the importing user) |
| image | photo |

Not imported: tools, comments, timeline events, meal plans and shopping lists.

## Tandoor

An export zip containing one zip per recipe, each with `recipe.json` (or any `.json`) and
`image.jpg|jpeg|png|webp|gif`. A single recipe zip, or `recipe.json` files directly in a zip,
also work. A JSON array, or an API response with `results`, can be imported as a `.json`
file.

| Tandoor | Larder |
|---|---|
| `name`, `description` | name, description |
| `servings`, `servings_text` | yield quantity and unit |
| `working_time` | prep time |
| `waiting_time` | cook time |
| `source_url` | source URL |
| `rating` | rating |
| `keywords[].name` | tags |
| `steps[]` (sorted by `order`) | steps: `instruction` split into paragraphs; a paragraph starting with `#` sets a section; the step `name` is the section |
| `steps[].ingredients[]` | ingredients: `amount`, `unit.name`, `food.name`, `note`; `no_amount` drops the amount; `is_header` rows start a section; if more than one step has ingredients, the step name is used as the ingredient section; rows without food and unit use `original_text` |
| `nutrition` (object of numbers, or text) | nutrition, with units kcal / mg / g added |
| `created_at` | creation date |
| `image.*` | photo |

## Paprika

A `.paprikarecipes` file (a zip of gzip-compressed JSON `.paprikarecipe` files), or a single
`.paprikarecipe` (gzip or plain JSON).

| Paprika | Larder |
|---|---|
| `name`, `description` | name, description |
| `servings` (or `yield`) | yield parsed from text |
| `prep_time`, `cook_time`, `total_time` | times parsed from text |
| `source_url` | source URL |
| `source` (unless it is a URL) | source name |
| `rating` | rating |
| `categories` | categories |
| `ingredients` (text) | ingredients, one per line; lines ending in `:` or starting with `#` are sections |
| `directions` (text) | steps: if two or more lines are numbered, numbered lines start steps and others continue them; otherwise each line is a step; short lines ending in `:` or starting with `#` are sections |
| `notes` | notes |
| `nutritional_info` | nutrition if it is `Name: value` lines; otherwise appended to notes under "Nutrition:" |
| `photo_data` (base64), else `image_url` | photo |
| `created` | creation date |
| `on_favorites` | favourite |

## Nextcloud Cookbook

A zip of the Nextcloud recipes folder: `<Recipe name>/recipe.json` with `full.jpg`. Any
`.json` file in the zip (except `manifest.json`) is read; all schema.org recipes in it are
imported. A `recipe.json` without `@type` but with `name` is treated as a recipe. The photo is
the first of `full`, `image`, `photo`, `cover` (`.jpg`, `.jpeg`, `.png`, `.webp`, `.gif`) in
the same folder, else any other image there except `thumb…` files; it is only attached when
the JSON file contains exactly one recipe.

Field mapping: as for [schema.org JSON-LD](#schemaorg-json-ld), below.

## Recipe Keeper

A zip with `recipes.html` and `images/`. Each `div.recipe-details` is one recipe; fields are
read from `itemprop` attributes (the `content` attribute if present, else the text).

| `itemprop` | Larder |
|---|---|
| `name` | name |
| `recipeYield` | yield parsed from text |
| `prepTime`, `cookTime`, `totalTime` | times |
| `recipeCategory` | categories |
| `recipeCourse`, `recipeCollection` | tags |
| `recipeSource` | source URL if it is a link, else source name |
| `recipeRating` | rating |
| `recipeIsFavourite` / `recipeIsFavorite` | favourite |
| `recipeIngredients` | ingredients, one per `<p>`/`<li>` (or line) |
| `recipeDirections` | steps, one per `<p>`/`<li>`; leading numbers removed; short lines ending in `:` are sections |
| `recipeNotes` | notes |
| `description` / `recipeDescription` | description |
| `recipeNut…` (Calories, TotalFat, …) | nutrition; mg added to sodium and cholesterol, g to other numbers |
| `photo0` (else the first `photo…`, else `img.recipe-photo`) | photo, from the zip or an http(s) URL |

## schema.org JSON-LD

Used for `.json` files, Nextcloud Cookbook, saved HTML pages and website imports. Recipes are
found anywhere in the document: top level, arrays, `@graph`, `mainEntity`, nested objects
(types such as `http://schema.org/Recipe` or `["Recipe", "NewsArticle"]` count).

| schema.org | Larder |
|---|---|
| `name` (or `headline`, `title`) | name |
| `description` | description |
| `recipeYield` (or `yield`, `servings`); for an array the most descriptive entry wins | yield |
| `prepTime` | prep time |
| `cookTime` (or `performTime`) | cook time |
| `totalTime` | total time |
| `recipeIngredient` (or `ingredients`, `recipeIngredients`): strings, a text block, or objects with `text`/`name` and `requiredQuantity`/`amount` | ingredients; lines ending in `:` are sections |
| `recipeInstructions` (or `instructions`, `recipeDirections`, `directions`, `step`): string, array, `HowToStep`, `HowToSection` with `itemListElement`, `ItemList`, HTML lists | steps; `HowToSection` names and headings become sections; step numbers and bullets removed; a step's `text` is preferred over its `name` |
| `url` (or `mainEntityOfPage`), else the page address | source URL |
| `publisher.name` | source name |
| `aggregateRating.ratingValue` / `bestRating` (scaled to 5), or `rating` | rating |
| `keywords`, `recipeCuisine` | tags |
| `recipeCategory` | categories |
| `nutrition` (`NutritionInformation`) | nutrition (values up to 100 characters) |
| `notes` / `comment` | notes |
| `image` (or `thumbnailUrl`, `photo`): URL, array (largest declared `width` wins) or `ImageObject` | photo |
| `dateCreated` | creation date (not for website imports) |

### Website imports

Web pages are read as JSON-LD first (tolerating HTML comments, CDATA wrappers, raw newlines
and trailing commas). If several recipes are present the most complete wins (ingredients and
instructions weigh most). `@id` references in `image`, `publisher`, `author`,
`recipeInstructions`, `nutrition` and `aggregateRating` are resolved through the page's
`@graph`. If the JSON-LD recipe has neither ingredients nor instructions, **microdata**
(`itemscope itemtype=".../Recipe"`) is tried. Missing values are filled from the page:
`og:title` / `twitter:title` / `<title>` for the name, `og:description` / `description` for
the description, the site's `WebSite`/`Organization` name, `og:site_name` or the host for the
source name, and `og:image` / `twitter:image` for the photo. Pages declared as ISO-8859-1 or
Windows-1252 are decoded accordingly; others as UTF-8.

## Larder backup

A zip with `manifest.json` (`{"format": "larder-export", "version": 1, …}`; the legacy
`recipers-export` is accepted; newer versions are refused), `recipes.json` (an array, or
`{"recipes": [...]}`, possibly in a sub-folder) and `images/`. Each entry has every recipe
field as in the [API](api.md#recipes), plus `id`, `image` (path inside the zip, or an http(s)
URL when importing plain JSON), `favorite`, `created_at` and `updated_at`. All fields are
kept; the id is not (imported recipes get new ids). `source_name` is not filled in
automatically for this format.

## Plain text and AI

Pasted text uses a heuristic parser (headings, metadata lines, numbered steps), described in
[Importing → Text](../guide/importing.md#text). With AI assist the text or photo is sent to
the language model, which returns name, description, yield, times, ingredient and step lines
(`## ` lines become sections), tags, categories and notes; see [AI assist](../guide/ai.md).

## Known gaps

- Only the formats above are supported. Other apps' HTML exports (for example a generic
  `recipes.html` without Recipe Keeper's markup) are refused.
- Mealie tools, Tandoor step times and per-step ingredient links, Paprika difficulty and
  photos beyond the main one, and comments are not imported.
- Only one photo per recipe is imported.
- Meal plans and shopping lists are never imported from other apps.
