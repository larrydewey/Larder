# Backup & restore

Any user can export their household's recipes as a zip and import it again, into the same
server, another server, or a standalone phone. Server administrators should also back up the
server itself; see [Backups & upgrades](../admin/backups.md).

## How to make a backup

Tap **Settings → Data → Export all as JSON backup** (also at the bottom of the **Export
cookbook** screen). You see "Preparing backup…" and then "Backup ready".

- **Web**: the browser downloads `larder-backup-YYYY-MM-DD.zip`.
- **Android**: the zip is saved in the app's `exports` folder and the share sheet opens. Save
  it to Files, Google Drive, email it to yourself, and so on.

In connected mode the backup is made by the server, so it needs a connection. In standalone
mode it is made on the phone.

## What is in the zip

```text
larder-backup-2026-11-09.zip
├── manifest.json     {"format": "larder-export", "version": 1, "exported_at": "...",
│                      "recipe_count": 42, "generator": "Larder 0.1.0"}
├── recipes.json      [ { "id": "...", "name": "...", ..., "image": "images/<id>.jpg",
│                         "favorite": true, "created_at": "...", "updated_at": "..." } ]
└── images/
    └── <recipe id>.jpg
```

Each recipe in `recipes.json` has all recipe fields (name, description, yield, times, source,
notes, rating, ingredients with sections and parsed parts, instructions, tags, categories,
nutrition), plus its id, the path of its photo in the zip, whether it is one of **your**
favourites, and its creation and update times. Photos are the full-size versions.

!!! warning "Recipes only"

    The backup contains **recipes and their photos**. It does not contain the meal plan,
    shopping lists, household members, other people's favourites or settings. A server backup
    (see [Backups & upgrades](../admin/backups.md)) contains everything.

## How to restore

1. Go to **Import → File**.
2. Set **Source** to **Larder backup (.zip)**, or leave **Detect automatically**.
3. Decide on **Skip recipes I already have**: leave it on to add only missing recipes; turn it
   off to import everything again.
4. Tap **Choose file & import** and pick the zip.

Restored recipes get **new ids** and are created by the user who imports them. Favourites
marked in the backup become favourites of the importing user; creation dates are kept.

Older backups whose manifest says `recipers-export` (from releases named "Recipe-rs") are
accepted too. A backup from a newer Larder version than the one importing it is refused
("Larder export version N is newer than supported"). A plain `recipes.json` without the zip
also imports, without photos.

## Moving between standalone and a server

You do not need backups for this. Use **Connect to a server…** (standalone → server) or
**Disconnect and use on this device only** (server → phone); see
[Standalone mode](standalone.md). Those also move the meal plan and shopping lists.
