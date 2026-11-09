# Standalone mode

Standalone mode is Android only. The app works with no server and no account: your recipes,
meal plan, shopping lists, photos and settings live on the phone.

## How it works

- Data is kept in a local database in the app's private storage. Photos are stored as JPEGs in
  three sizes (320, 960 and 2400 px) under the app's documents folder (`images/`).
- Everything the server would do is done by **`liblarder_engine.so`**, a native library built
  from the same Rust code as the server: ingredient parsing and recipe normalisation, imports
  (files, websites, text, Mealie), shopping-list merging and aisle classification, AI assist,
  the PDF cookbook and backups. Results are the same as on a server.
- The engine is bundled in release APKs for `arm64-v8a`, `armeabi-v7a` and `x86_64`.

## Choosing standalone mode

On the **Welcome to your larder** screen tap **Use on this device**, or on the server address
screen tap **Use on this device instead**. The app checks that the engine loads first; if it
cannot, you see **On-device mode unavailable** ("This build cannot run without a server.")
with the technical reason, and stay where you were.

## What is different from connected mode

| | Standalone |
|---|---|
| Settings | No account, household, sync or sign-out sections. Profile card **On this device**. Adds **AI assistant**. |
| Units preference | Stored on the device |
| Imports | Run on the phone; the phone connects to websites and Mealie directly. Import history is kept on the phone (last 50). |
| AI assist | Your own API key, see [AI assist](ai.md) |
| Cookbook PDF, backups | Rendered on the phone; large books can take a while |
| Offline | Always works; there is nothing to sync |
| Search | Also matches the source name |

!!! warning "Back up"

    The phone holds the only copy. Uninstalling the app or clearing its storage deletes your
    recipes. Export a [backup](backup.md) regularly.

## Connecting to a server later

You can move everything from the phone to a server at any time.

1. **Settings → Connect to a server…**
2. **Connect to a server**: enter the **Server URL** and tap **Connect** ("Your recipes, meal
   plan and shopping lists will be copied to the server after you sign in."). **Cancel** goes
   back.
3. Sign in, or create an account (with an invite code to join an existing household), or use
   single sign-on.
4. **Copy your data to the server**: "Recipes (with photos), your meal plan and shopping lists
   will be uploaded. Afterwards this device stays in sync with the server."
    - **Skip recipes whose name already exists on the server** (on by default): a recipe is
      not uploaded if your household on the server already has one with the same source URL,
      or the same name ignoring case and accents (the same rule as
      [import duplicate detection](importing.md#duplicates)). Meal plan entries that used it
      are pointed at the server's recipe instead.
    - Tap **Start**. Progress is shown per phase: "Recipes · 12 / 40", "Meal plan …",
      "Shopping lists …".
5. **All set!** shows what was copied ("40 recipes, 35 photos, 12 meal plan entries, 2 lists
   and 31 items copied; 3 recipes already existed.") and expandable "N items could not be
   copied" warnings. Tap **Open my recipes**.

What is uploaded:

- every recipe, **keeping its id**, with its favourite flag (for your account) and full-size
  photo;
- every meal plan entry;
- every shopping list and its items, including checked state and aisle.

The upload is **resumable**. Each finished item is recorded on the phone. If the connection
drops you see "Lost the connection to the server. Your progress is saved; try again to
resume." and the button becomes **Resume**. Leaving and coming back shows "A previous copy was
interrupted; it will continue where it stopped." Because ids are kept, re-sending something
that already arrived does not create a duplicate. Items the server rejects (for example
invalid data) are skipped and listed as warnings; network and server errors stop the copy so
you can resume.

After **Open my recipes** the app switches to connected mode and does a full sync, replacing
the local copy with the server's (which now contains everything).

**Cancel and stay on this device** signs out of the server and keeps you in standalone mode
with your data untouched.

## Disconnecting from a server

In connected mode on Android, **Settings → Disconnect and use on this device only** turns the
cached server data into a standalone collection.

The **Use on this device only?** dialog explains: "Your recipes, meal plan and shopping lists
stay on this phone and photos are downloaded for offline use. Changes will no longer sync with
the server or your household." Tap **Disconnect**:

1. Pending changes are sent to the server first ("Saving pending changes"). If some cannot be
   sent (for example you are offline), the dialog says "N changes could not be sent to the
   server." and offers **Discard them and continue** or **Cancel**.
2. Every recipe photo is downloaded at full size and stored on the phone ("Downloading photos ·
   12 / 40"). A photo that fails to download is skipped; the recipe is kept without it.
3. The app switches to standalone mode and signs out of the server: "Disconnected. Your
   recipes now live on this device."

What you keep is what was cached: all of the household's recipes, the meal plan from 30 days
ago to 60 days ahead, and all shopping lists. Nothing is deleted on the server; your household
keeps using it. You can connect again later with **Connect to a server…**.
