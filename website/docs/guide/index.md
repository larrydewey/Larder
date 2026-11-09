# User guide

This guide covers every screen of the app. The Android app and the web UI are the same Flutter
app, so the instructions apply to both; where they differ, it is called out.

## Finding your way around

The app has five main sections. On phones they are in the navigation bar at the bottom; on
screens 720 px wide or more they move to a navigation rail on the left, which shows the Larder
name next to the logo from 1180 px.

| Section | What it is for |
|---|---|
| **Recipes** | Browse, search and filter your recipes; open one to cook, scale, plan or edit it. |
| **Meal Plan** | The week ahead, day by day. |
| **Shopping** | Your shopping lists. |
| **Import** | Bring recipes in from websites, text, photos, files and Mealie, and see import history. |
| **Settings** | Account, preferences, household, data exports, server connection. |

The **Export cookbook** screen (the printable PDF) opens from the recipe list's **⋮** menu or
from **Settings → Data**.

## Web and Android differences at a glance

| Feature | Web | Android |
|---|---|---|
| Exports (PDF, backup zip) | Downloaded by the browser | Saved in the app's `exports` folder and offered through the share sheet |
| Photos | Browser file chooser (the **Camera** button may also open a file chooser, depending on the browser) | Gallery picker or camera |
| Single sign-on | The tab goes to the identity provider and returns to `/oidc-callback` | A browser tab opens and returns through `larder://auth` |
| Standalone mode | Not available | Available |
| Change server | Type a new URL; data for the old server is cleared | Signs out and forgets the server (with confirmation) |
| Timer alarm | Three beeps through the browser (may be blocked until you interact with the page) | System alert sound and vibration |
| Offline cache | Browser storage (IndexedDB) | App database on the phone |

## Pages in this guide

- [Recipes](recipes.md): the list, search, filters and the recipe page.
- [Editing recipes](editing.md): every field of the editor.
- [Organizing recipes](organize.md): automatic categories and tidying ingredients in bulk.
- [Scaling & units](scaling.md): servings, metric and imperial, fractions.
- [Cook mode](cook-mode.md): step-by-step view and timers.
- [Meal planner](meal-plan.md)
- [Shopping lists](shopping.md)
- [Importing recipes](importing.md): a walkthrough for every source.
- [Printable cookbook](cookbook.md)
- [Backup & restore](backup.md)
- [AI assist](ai.md)
- [Households & sharing](households.md)
- [Settings](settings.md)
- [Standalone mode](standalone.md)
- [Offline use & sync](offline.md)
