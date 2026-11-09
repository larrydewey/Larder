# Offline use & sync

In connected mode Larder is **offline-first**. The app keeps a copy of your household's data
on the device and reads everything from that copy, so it opens instantly and keeps working
without a connection.

## What is kept on the device

| Data | Kept |
|---|---|
| Recipes | All of the household's recipes |
| Photos | Downloaded and cached as you view them (connected mode); not all photos are guaranteed to be available offline |
| Meal plan | From 30 days ago to 60 days ahead (other weeks are fetched when you open them) |
| Shopping lists | All lists with all items |
| Your profile and server settings | Yes |

On Android this is the app's database; on the web it is the browser's IndexedDB storage for
the server's address.

## Making changes offline

These changes are saved locally at once and queued in an **outbox**:

- creating, editing, deleting, favouriting recipes;
- adding, editing, moving and removing meal plan entries;
- creating, renaming and deleting lists; adding, editing, checking, deleting and clearing
  items; adding recipes or a week of meals to a list.

You see "Saved offline. It will sync when you are back online." The queue is sent in order as
soon as the server can be reached.

These need a connection and say "This needs a connection to the server." when offline:
photos (adding, changing, removing), duplicating a recipe, imports, the cookbook PDF and
backups, household and admin screens, profile and password changes.

## The banner

A thin banner at the top of the main screens tells you what is going on:

- **Offline · showing saved recipes**: no connection, nothing waiting.
- **Offline · 3 changes waiting to sync**: no connection, changes queued.
- **Syncing 3 changes…**: connected, sending.

Tap the banner (**Retry**) to try again immediately.

## When sync happens

The app sends queued changes and then fetches updates:

- when the app starts and when it returns to the foreground;
- when the device's network connection comes back;
- when you pull down to refresh a list, or tap **Sync now** (Settings or the recipe list
  **⋮** menu);
- every 45 seconds while changes are waiting;
- right after you make a change, if online.

Updates are incremental: the app asks the server for what changed since the last sync (with a
10-second overlap to be safe). New devices and changed accounts get a full copy.

## Conflicts and rejected changes

Changes are replayed in the order you made them. For each one:

| Server answer | What the app does |
|---|---|
| Accepted | Applies the server's response to the local copy. |
| **409 Conflict** (the recipe was changed by someone else since you started editing) | **The server wins.** The app drops your change, loads the server's version and shows "*Saving "Pancakes"* was changed by someone else; the server version was kept." |
| Deleting something that is already gone (404) | Treated as success. |
| Other refusal (4xx), for example invalid data or a deleted list | Drops the change, shows "*…* failed: *reason*" and syncs again to restore the server's state. |
| Server error (5xx) | Stops and retries later. After 5 failed attempts the change is dropped with a message. |
| No connection | Stops and retries later. |
| Session expired | Stops; you are asked to sign in again. |

If you are online and editing when someone else saves the same recipe, the editor asks you
directly: **Reload** or **Overwrite with mine**. See
[Editing recipes](editing.md#saving-and-conflicts).

Creating things offline is safe to repeat: every recipe, meal plan entry, list and item gets
its id on the device, and the server treats a second create with the same id as the same item.

## Signing out with pending changes

Signing out deletes the local copy **and the outbox**. If changes are waiting you are warned:
"You have N changes that have not synced yet. They will be lost." Sync first.

## Several devices

Each device has its own cache and outbox. Changes from one device reach another the next time
it syncs (for example when you bring the app to the foreground). There are no live push
updates.
