# Choosing how to run Larder

There are three sensible set-ups. You can move from one to another later without losing data.

## Standalone Android

Install the APK and choose **Use on this device** on the first screen. Everything lives in a
local database on the phone and photos are stored in the app's documents folder. All the logic
that normally runs on the server (ingredient parsing, imports, shopping-list merging, the PDF
cookbook, AI assist) runs on the phone through `liblarder_engine.so`, a library built from the
same Rust code as the server.

Good for: one person, one phone, no infrastructure.

## Self-hosted server

Run the `larder` server (Docker or a binary) and use it from any browser at its address, and
optionally from the Android app in connected mode. Everyone in a household shares recipes,
meal plans and shopping lists.

Good for: families and anyone who wants to use a laptop or desktop browser.

## Both

Use the Android app in **connected** mode against your server. The app keeps a full local
copy of your household's recipes, the meal plan around today and all shopping lists, so it
works in the supermarket with no signal. Changes made offline are queued and sent when the
connection returns. See [Offline use & sync](../guide/offline.md).

You can also start standalone and **connect later** (Settings → **Connect to a server…**):
the app uploads everything to the server once, then switches to connected mode. Or go the
other way and **disconnect** (Settings → **Disconnect and use on this device only**), which
keeps your data and downloads the photos to the phone. See
[Standalone mode](../guide/standalone.md).

## Comparison

| | Standalone Android | Connected Android | Web |
|---|---|---|---|
| Needs a server | No | Yes | Yes (the server serves the web app) |
| Account | None | Server account (password or SSO) | Server account (password or SSO) |
| Share with a household | No | Yes | Yes |
| Works offline | Always | Yes: cached data, queued changes | Only while the tab is open; data is cached in the browser |
| Where data lives | Phone (local database + photo files) | Server; cached copy on the phone | Server; cached copy in browser storage (IndexedDB) |
| Imports (files, URLs, text) | On the phone | On the server | On the server |
| Mealie live import | From the phone to Mealie | From the server to Mealie | From the server to Mealie |
| AI assist (text, photos) | With your own API key, set in Settings | If the server admin enabled it | If the server admin enabled it |
| Printable cookbook PDF | Rendered on the phone | Rendered on the server | Rendered on the server |
| Photo handling | Resized to 320 / 960 / 2400 px JPEGs on the phone | Uploaded and resized by the server | Uploaded and resized by the server |
| Unit preference | Stored on the device | Stored in your server profile | Stored in your server profile |
| Backups | **Export all as JSON backup** (zip) | Server backups plus the same zip export | Server backups plus the same zip export |
| Households, invites, admin | Not applicable | Yes | Yes |
| Single sign-on (OIDC) | Not applicable | Yes, through the system browser | Yes |

!!! warning "Standalone means one copy"

    In standalone mode the phone holds the only copy of your recipes. Use
    **Settings → Export all as JSON backup** regularly and keep the zip somewhere safe, or
    connect to a server.

## What the server needs

- A small x86-64 machine: the server is a single binary, and a small NAS or any VPS is enough
  for a household. PDF rendering and large imports are the heaviest operations. The published
  image is `linux/amd64` only; on ARM, such as a Raspberry Pi, build the binary from source.
- Docker (recommended) or a Linux machine to run the binary.
- Optional: PostgreSQL. SQLite is the default and fine for households.
- Optional: a domain name and a reverse proxy for HTTPS, needed if you want to reach it from
  outside your home network or use single sign-on. Plain `http://` works on a LAN; the Android
  app allows cleartext HTTP for this reason.

Next: [Install the server](server.md) or [Install the Android app](android.md).
