# Single sign-on (OIDC)

Larder can let people sign in with an OpenID Connect identity provider such as Authentik,
Keycloak, Authelia, Pocket ID or Zitadel. Local username/password accounts keep working
alongside it.

## How it works

1. The user taps **Sign in with *Name*** on the sign-in screen.
2. The app opens `GET /api/auth/oidc/login?redirect_uri=…` on your server:
    - **Web**: the browser tab navigates there, with `redirect_uri=<your site>/oidc-callback`.
    - **Android**: a browser tab opens, with `redirect_uri=larder://auth`.
3. Larder redirects to the provider using the **authorization code flow with PKCE (S256)**,
   a random `state` and `nonce`, and the scopes from `LARDER_OIDC_SCOPES`. The provider is
   told to return to **`${LARDER_PUBLIC_URL}/api/auth/oidc/callback`**.
4. The provider redirects back to that callback. Larder exchanges the code at the token
   endpoint (sending the client secret if configured), checks the `nonce` in the ID token,
   and reads the user's claims from the **userinfo endpoint**.
5. Larder finds or creates the account (see below) and redirects to the app's `redirect_uri`
   with the tokens in the URL fragment (`#access_token=…&refresh_token=…&expires_in=900`), or
   `#error=…` on failure. The web app removes the tokens from the address bar immediately.

The login attempt must be completed within **10 minutes**. Pending attempts are kept in the
server's memory, so a server restart in the middle of a sign-in makes it fail with "unknown or
expired login attempt; please try again".

## Server configuration

```bash
LARDER_PUBLIC_URL=https://recipes.example.com
LARDER_OIDC_ISSUER=https://auth.example.com/...      # see per-provider sections
LARDER_OIDC_CLIENT_ID=larder
LARDER_OIDC_CLIENT_SECRET=...                       # omit for a public client
LARDER_OIDC_NAME=Authentik                          # button label
# optional:
LARDER_OIDC_SCOPES="openid profile email"
LARDER_OIDC_AUTO_REGISTER=true
LARDER_OIDC_USERNAME_CLAIM=preferred_username
LARDER_OIDC_ADMIN_GROUP=larder-admins
```

In the provider, register:

| Setting | Value |
|---|---|
| Client type | Confidential (with a secret) is recommended; public with PKCE also works |
| Grant type | Authorization code |
| **Redirect URI** | `https://recipes.example.com/api/auth/oidc/callback` (that is, `${LARDER_PUBLIC_URL}/api/auth/oidc/callback`, exactly) |
| Scopes | `openid profile email` (plus `groups` if you use the admin group and your provider needs it) |
| Token endpoint authentication | Client secret sent in the request body (`client_secret_post`) |

!!! info "Only one redirect URI"

    The provider only ever redirects to the server's `/api/auth/oidc/callback`. You do **not**
    register `larder://auth` or `/oidc-callback` in the provider: those are where Larder
    itself sends the user afterwards. Larder only allows these targets: `larder://…`, paths on
    the server (`/…`), URLs under `LARDER_PUBLIC_URL`, and origins in `LARDER_CORS_ORIGINS`.
    Anything else gets "redirect_uri is not allowed".

Larder needs the provider's discovery document to contain `authorization_endpoint`,
`token_endpoint` and `userinfo_endpoint`.

## Accounts

When the provider returns a user, Larder decides who it is in this order:

1. **Already linked.** An account linked to this provider and subject (`sub`) signs in. If it
   is disabled: "this account is disabled".
2. **Linked by verified email.** If the userinfo has an `email` and `email_verified` is
   `true`, and a local account with that email (ignoring case) is not yet linked to any
   provider, the account is linked and signs in. A disabled account is refused ("this account
   is disabled") and is not linked. From then on the user can use either password or SSO.
3. **Auto-registration.** If `LARDER_OIDC_AUTO_REGISTER` is true (the default), a new account
   is created:
    - **Username**: the claim named by `LARDER_OIDC_USERNAME_CLAIM`, else
      `preferred_username`, else `nickname`, else the part of the email before `@`, else
      `user`. Characters other than letters, digits, `_`, `-` and `.` are removed and it is
      cut to 48 characters. If taken, `2`, `3`… is appended.
    - **Display name**: the `name` claim, else the username.
    - **Email**: from the userinfo, unless another account already uses it.
    - **Household**: if the user entered an **invite code** before tapping the SSO button
      (on the **Create an account** form), they join that household as a member; an invalid or
      expired code fails with "invalid or expired invite code". Otherwise they get a new
      household of their own as owner.
    - **No password.** They can set one later in **Settings → Change password** (leaving
      **Current password** empty).
    - Auto-registration is independent of `LARDER_ALLOW_REGISTRATION`: with auto-registration
      on, anyone who can sign in at your provider can create a Larder account.
4. Otherwise: "no account is linked to this identity".

If nobody has registered yet, the first account created through SSO becomes server admin, as
with a password account. Note that the sign-in screen of a brand-new server shows **Create
admin account** without the SSO button, so in practice create the first account with a
password.

### Admin group

If `LARDER_OIDC_ADMIN_GROUP` is set and the userinfo response has a `groups` array containing
that exact value, the user is made a server admin when their account is created, when an
existing account is linked by email, and every time they sign in with an already linked
account. Removing someone from the group does **not**
remove their admin flag; do that in **Settings → Server users**.

!!! warning "Groups must be in the userinfo response"

    Larder reads `groups` from the **userinfo endpoint**, not from the ID token. Configure your
    provider to include groups there, and request the `groups` scope if your provider needs it.

## Provider setup

Replace `recipes.example.com` and `auth.example.com` with your own host names.

=== "Authentik"

    1. **Applications → Providers → Create → OAuth2/OpenID Provider**.
        - Client type: **Confidential**.
        - Redirect URIs: `https://recipes.example.com/api/auth/oidc/callback` (strict).
        - Scopes: keep the default `openid`, `email` and `profile` mappings. Authentik's
          `profile` scope includes a `groups` claim.
    2. **Applications → Create**, slug `larder`, using that provider.
    3. Copy the client ID and secret.

    ```bash
    LARDER_OIDC_ISSUER=https://auth.example.com/application/o/larder
    LARDER_OIDC_CLIENT_ID=<client id>
    LARDER_OIDC_CLIENT_SECRET=<client secret>
    LARDER_OIDC_NAME=Authentik
    LARDER_OIDC_ADMIN_GROUP=larder-admins      # an Authentik group name
    ```

    Account linking needs `email_verified: true` in the userinfo. Check what your Authentik
    version's email scope mapping returns; if it reports `false`, linking by email does not
    happen and a new account is created instead (or refused, with auto-registration off).

=== "Keycloak"

    1. In your realm, **Clients → Create client**, type **OpenID Connect**, client ID
       `larder`.
    2. **Client authentication: On** (confidential), **Standard flow: On**.
    3. **Valid redirect URIs**: `https://recipes.example.com/api/auth/oidc/callback`.
    4. Copy the secret from **Credentials**.
    5. For the admin group: **Client scopes → larder-dedicated → Add mapper → By
       configuration → Group Membership**, token claim name `groups`, **Full group path:
       Off**, **Add to userinfo: On**.

    ```bash
    LARDER_OIDC_ISSUER=https://auth.example.com/realms/<realm>
    LARDER_OIDC_CLIENT_ID=larder
    LARDER_OIDC_CLIENT_SECRET=<secret>
    LARDER_OIDC_NAME=Keycloak
    LARDER_OIDC_ADMIN_GROUP=larder-admins
    ```

    Keycloak sets `email_verified` from the user's **Email verified** flag.

=== "Authelia"

    Add a client to Authelia's configuration:

    ```yaml title="configuration.yml"
    identity_providers:
      oidc:
        clients:
          - client_id: larder
            client_name: Larder
            client_secret: '$pbkdf2-sha512$...'   # hash of the secret
            public: false
            authorization_policy: two_factor
            redirect_uris:
              - https://recipes.example.com/api/auth/oidc/callback
            scopes: [openid, profile, email, groups]
            grant_types: [authorization_code]
            response_types: [code]
            token_endpoint_auth_method: client_secret_post
    ```

    ```bash
    LARDER_OIDC_ISSUER=https://auth.example.com
    LARDER_OIDC_CLIENT_ID=larder
    LARDER_OIDC_CLIENT_SECRET=<plain secret>
    LARDER_OIDC_NAME=Authelia
    LARDER_OIDC_SCOPES="openid profile email groups"
    LARDER_OIDC_ADMIN_GROUP=larder-admins
    ```

    `token_endpoint_auth_method: client_secret_post` matters: Larder sends the secret in the
    form body. Depending on your Authelia version and claims policy, `email`, `groups` and
    other claims may only be in the ID token rather than the userinfo response; Larder reads
    the userinfo, so make sure they are released there.

=== "Pocket ID"

    1. **OIDC Clients → Add OIDC Client**, name `Larder`.
    2. Callback URL: `https://recipes.example.com/api/auth/oidc/callback`.
    3. Copy the client ID and secret.
    4. For the admin group, create a user group (for example `larder-admins`) and add members.

    ```bash
    LARDER_OIDC_ISSUER=https://id.example.com
    LARDER_OIDC_CLIENT_ID=<client id>
    LARDER_OIDC_CLIENT_SECRET=<client secret>
    LARDER_OIDC_NAME="Pocket ID"
    LARDER_OIDC_SCOPES="openid profile email groups"
    LARDER_OIDC_ADMIN_GROUP=larder-admins
    ```

    Pocket ID is passkey-only, which pairs well with Larder's password-less SSO accounts.

## Troubleshooting

| Symptom | Cause |
|---|---|
| Provider says the redirect URI is invalid / mismatched | The provider's redirect URI is not exactly `${LARDER_PUBLIC_URL}/api/auth/oidc/callback`. Check scheme, host, port and that `LARDER_PUBLIC_URL` has no path. |
| "redirect_uri is not allowed" (from Larder) | You opened the web app at an address other than `LARDER_PUBLIC_URL` (for example `http://192.168.1.10:8080` while the public URL is `https://recipes.example.com`). Use the public address, or add the other origin to `LARDER_CORS_ORIGINS`. |
| "OIDC discovery failed" | The server cannot reach `${LARDER_OIDC_ISSUER}/.well-known/openid-configuration`. Check the issuer URL and DNS/TLS from inside the container. |
| "token exchange failed: …" | Wrong client secret, or the provider expects a different token authentication method. |
| "OIDC nonce mismatch" | The provider returned an ID token for another login attempt; try again. |
| "no account is linked to this identity" | Auto-registration is off and no account matched by subject or verified email. |
| "unknown or expired login attempt; please try again" | More than 10 minutes passed, the server restarted, or the callback was opened twice. |
| Android returns to the app but nothing happens | Make sure you are on the latest APK; the callback scheme is `larder://auth`. |
| Admin group has no effect | `groups` is not in the userinfo response, or the value does not match exactly (Keycloak with full group path gives `/larder-admins`). |
