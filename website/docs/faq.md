# FAQ & troubleshooting

## Accounts and signing in

### "Registration is closed. Ask the household owner for an invite code."

The server does not allow open sign-up (`LARDER_ALLOW_REGISTRATION` is `false`, the default)
and you did not enter an invite code. Ask a household owner for a code
(**Settings → Household → New invite**) and enter it in **Invite code** on the **Create an
account** form. Administrators who want anyone to be able to register can set
`LARDER_ALLOW_REGISTRATION=true`.

### "invalid or expired invite code"

The code is mistyped, was revoked, or has expired (invites last 1, 7 or 30 days). Codes are
not case-sensitive and never contain `0`, `O`, `1` or `I`. Ask for a new one. The same message
appears after single sign-on if the code entered before tapping **Sign in with …** is invalid.

### "Wrong username or password."

Check the username (or email) and password. Usernames are not case-sensitive, passwords are.
If you forgot the password, ask a server admin to use **Reset password** in
**Settings → Server users**; there is no email reset.

### "this account is disabled"

A server admin disabled your account. Only an admin can enable it again.

### "username or email is already taken"

Someone on the server already uses that username or email (compared without regard to case).
Choose another, or leave the email empty.

### Changing my password signed me out on my other devices

That is intended: a new password revokes every other session of the account, so a lost or
shared device loses access. The device you changed it on stays signed in. Sign in again with
the new password elsewhere.

## Single sign-on

### The identity provider says the redirect URI does not match

Register exactly `${LARDER_PUBLIC_URL}/api/auth/oidc/callback` in the provider, for example
`https://recipes.example.com/api/auth/oidc/callback`, and make sure `LARDER_PUBLIC_URL` is the
public HTTPS address without a path or trailing slash. Larder always sends this callback URL,
whether you sign in on the web or on Android. See [OIDC](admin/oidc.md).

### "redirect_uri is not allowed"

You opened the web app at a different address than `LARDER_PUBLIC_URL` (for example the LAN
IP instead of the domain). Open it at the public address, or add the other origin to
`LARDER_CORS_ORIGINS`.

### "unknown or expired login attempt; please try again"

The sign-in took more than 10 minutes, the server restarted during it, or the callback page
was reloaded. Start again.

### "no account is linked to this identity"

`LARDER_OIDC_AUTO_REGISTER` is `false`, and the provider's user is not linked to a Larder
account. To link an existing account automatically, the provider must report the same email
with `email_verified: true`. Otherwise turn auto-registration on.

### The admin group does not make me an admin

Larder reads `groups` from the provider's **userinfo** response, and the value must match
`LARDER_OIDC_ADMIN_GROUP` exactly. See [OIDC → Admin group](admin/oidc.md#admin-group).

## Sync and offline

### "… was changed by someone else; the server version was kept."

You (or the queue on your device) tried to save a recipe based on an older version while
someone else had saved a newer one. Larder keeps the server's version and discards your
change. Open the recipe and apply your edit again. When this happens while you are editing
online, the editor offers **Reload** or **Overwrite with mine** instead. See
[Offline use & sync](guide/offline.md#conflicts-and-rejected-changes).

### The banner says changes are waiting to sync, and they never go

- **Offline**: the app cannot reach the server. Check the connection and the server; tap the
  banner to retry. The app retries every 45 seconds and whenever the network comes back.
- **Server errors**: a change that fails with a server error is retried up to 5 times and then
  dropped with a "… failed" message. Check the server logs.
- **Signed out**: if the session expired, sign in again; queued changes are kept until you
  sign out.
- **Settings → Sync now** shows the last error.

If a change is stuck permanently, signing out discards the queue (you are warned how many
changes will be lost).

### "This needs a connection to the server."

Some actions only work online: photos, duplicating, imports, the PDF cookbook and backups,
household and admin screens, profile and password changes. Try again when connected.

### I added a recipe to a shopping list offline, but the list did not change

Adding recipes or a week of meals is queued while offline and the merged items appear after
the next successful sync.

## Recipes and photos

### Photos do not show

- **In the app on another device or browser**: photos are fetched from
  `<server>/api/images/<id>`. If the server address used by the app is not reachable from
  that device (for example `localhost`), photos fail. Use an address everyone can reach.
- **After restoring a backup** of only the database: photos are files in the data
  directory's `images/` folder; restore it as well.
- **Behind a reverse proxy**: make sure `/api/images/` is proxied like the rest of `/api`.
- **Offline**: only photos you have viewed before are cached in connected mode.
- **"unsupported image format"** when uploading: use JPEG, PNG, WebP or GIF. HEIC photos from
  iPhones must be converted. Images over 80 megapixels are refused.

### My photo upload fails with a large file

The server accepts request bodies up to `LARDER_MAX_UPLOAD_MB` (200 MB by default), and your
reverse proxy may have a lower limit (nginx's default is 1 MB). See
[Reverse proxy](admin/reverse-proxy.md).

### My ingredient lines look different after saving

Larder rewrites every ingredient line it understands in one standard form when a recipe is
saved or imported: "2 Tablespoons butter, melted" becomes "2 tbsp butter, melted" and "3 large
Eggs" becomes "3 eggs, large". Lines it cannot read reliably are kept as written. See
[How Larder tidies ingredient lines](guide/editing.md#how-larder-tidies-ingredient-lines).

### Imported recipes got categories I did not set

Recipes imported without a category get the one Larder suggests. Turn off **Sort recipes into
categories** for file and Mealie imports, or change the category in the editor. See
[Organizing recipes](guide/organize.md).

### Some ingredients are not scaled or converted

Only lines with a parsed quantity and food are scaled, and only volume and weight units are
converted. "Salt to taste" or lines the parser could not read are shown as written. Edit the
line into the form `quantity unit food, note`. See
[Ingredient parsing](reference/ingredients.md).

### An item ended up in the wrong aisle

Aisles are guessed from keywords (see [Aisles](reference/ingredients.md#aisles)); a food no
keyword matches goes to **Other**. Tap **✎** on the item and choose the right **Aisle**.

## Imports

### An import job "failed" after a server restart

Background imports run inside the server process. If it restarts, unfinished jobs are marked
failed with "The server restarted before the import finished". Recipes already imported are
kept. Run the import again with **Skip recipes I already have** on.

### "unrecognised import file" / "the archive does not contain a known recipe export"

The file is not one of the supported formats, or it was re-packed. Upload the file exactly as
the other app exported it, and try choosing the **Source** explicitly. See
[Import formats](reference/import-formats.md#detection).

### "no recipe found" when importing a website

The page has no schema.org recipe markup, or the site blocked the request. Copy the recipe
text and use the **Text** tab.

### "this address cannot be imported from"

The URL (or a redirect it leads to) points at the server itself (`localhost`, loopback), a
link-local or an unspecified address, which the server refuses for security. You may also see
"refusing to connect to '…': it resolves to a local or link-local address" or "redirect to a
local or link-local address refused". LAN addresses are allowed. If a Mealie server or website
runs on the same machine as Larder, use its LAN address instead of `localhost`. See
[Security](admin/security.md#importing-from-urls).

### "unauthorized: the server rejected the API token" (Mealie)

The Mealie API token is wrong, expired or was deleted. Create a new one in Mealie under your
user profile's API tokens.

### "too large: …"

The file or an entry in it exceeds a safety limit: 100 MB per file inside a zip, 1 GB in
total after decompression, 100 MB for non-zip files, or the upload limit. Split the export.

### Some recipes were skipped

With **Skip recipes I already have** on, recipes with the same source URL, or the same name
(ignoring case and accents), as an existing recipe are skipped. This rule is the same on the
server, on the phone and when connecting a phone to a server. Turn it off to import them
anyway.

## AI assist

### "AI features are not enabled on this server"

The server has no `LARDER_LLM_PROVIDER` configured, so the **Photo** tab and the AI switch are
hidden and AI API calls fail. An administrator can enable it; see [AI assist](guide/ai.md). In
standalone mode, set up a provider in **Settings → AI assistant** ("Set up an AI provider in
Settings to use AI imports.").

### The server does not start: "unknown LLM provider"

`LARDER_LLM_PROVIDER` must be `anthropic`, `claude`, `openai`, `openai-compatible`, `ollama`,
`none`, `off` or empty.

## Printable cookbook

### The PDF has empty boxes instead of characters

The bundled fonts cover Latin, Greek and Cyrillic. Chinese, Japanese, Korean, Arabic,
Devanagari and other scripts need extra fonts: on the server set `LARDER_COOKBOOK_FONT_DIRS`
to a directory with suitable `.ttf`/`.otf` fonts (for example Noto Sans CJK) and restart. This
is not possible in standalone mode. See
[Fonts and languages](guide/cookbook.md#fonts-and-languages).

### Generating the PDF times out

Large books with many photos take a while, all in one request. Raise your reverse proxy's
read timeout (several minutes), or untick **Photos**, or print a selection of recipes.

### There are blank pages in the PDF

They are intentional: chapters, the contents and the index start on right-hand pages for
double-sided printing. See [Printing and binding](guide/cookbook.md#printing-and-binding).

## Android

### "App not installed" when updating

Android refuses an update signed with a different key. Releases built with the project's
release key update each other, but CI artifacts and builds made without the signing secrets
are signed with a debug key and cannot update a release install (or the other way round).
Make sure everything is synced or backed up, uninstall the old app, and install the new APK.

### "On-device mode unavailable"

The APK was built without `liblarder_engine.so` for your phone's CPU. Use an official release
APK for your architecture (most phones: `arm64-v8a`), or connect to a server instead.

### "Could not reach a Larder server at that address."

Check the address and port (for example `http://192.168.1.10:8080`), that the phone is on the
same network, and that a firewall allows the port. Opening `<address>/api/config` in the
phone's browser should show JSON.

### The timer did not ring

Timers only run while the app is open; Android may stop a backgrounded app. Keep cook mode on
screen (it keeps the display awake).

## Server

### The server fails to start with a permission error on `/data`

The container runs as uid 10001. A bind-mounted host directory must be writable by it:
`chown 10001 /srv/larder`.

### "unsupported database url (expected sqlite:// or postgres://)"

`LARDER_DATABASE_URL` must start with `sqlite:`, `postgres://` or `postgresql://`.

### Visiting the server shows "Larder API server. The web UI is not installed"

The server did not find the web build. Set `LARDER_WEB_DIR` to the directory containing
`index.html` (the Docker image sets it to `/app/web`).

### Everyone was signed out after moving the server

If the new server has a different `jwt_secret` (or `LARDER_JWT_SECRET`), existing access
tokens become invalid, but apps refresh them automatically. If refresh tokens are gone too
(database not restored), everyone has to sign in again. Restore the data directory
completely when moving.
