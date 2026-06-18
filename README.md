# authstack

Headless, multi-tenant authentication service. Built with Rust, Axum, and PostgreSQL.

Acts as the auth backend for multiple applications — similar in spirit to Clerk or Kinde, but self-hosted. Each application is an isolated tenant: users, organizations, and credentials are fully scoped per app.

## Documentation

- [Authstack documentation](docs)

## How it works

Every application registers with Authstack and receives an `id` (TypeID, e.g. `app_01j4hz...`) and a `client_secret`. Backend services (BFFs) authenticate all requests to Authstack using HTTP Basic auth with those credentials. Authstack never talks directly to a browser or mobile client — all calls come through the app's backend.

```
Mobile / Web  →  App Backend (BFF)  →  Authstack
                 holds client_secret     issues JWTs
```

On signup, Authstack automatically creates a **personal organization** for the user within that app. Team organizations can be created on top of this. The issued JWT always carries `org_id`, `org_type`, and `role` so the consuming app has full access context without a second round-trip.

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/) and [Docker Compose](https://docs.docker.com/compose/) (v2)
- An ES256 key pair (see [Key generation](#key-generation))

> No local Rust or PostgreSQL installation required — everything runs in containers.

## Quick start

```bash
# 1. Generate an ES256 key pair
make keys

# 2. Copy the output into a .env file
cp .env.example .env
# Edit .env and paste in JWT_PRIVATE_KEY and JWT_PUBLIC_KEY

# 3. Start the database and API (creates the first admin on a fresh database)
make up
make logs
```

The API is available at **http://localhost:8080** once `make up` completes. On a fresh database, the container entrypoint runs `authstack bootstrap-admin` using `AUTHSTACK_BOOTSTRAP_EMAIL` and `AUTHSTACK_BOOTSTRAP_PASSWORD` from your environment (see [Bootstrap](#bootstrap)).

## Key generation

Authstack uses ES256 (ECDSA P-256) for JWTs. Run:

```bash
make keys
```

This prints two lines ready to paste into `.env`:

```
JWT_PRIVATE_KEY=LS0tLS1CRUdJTiBQUklWQVRFIEtFWS0tLS0t...
JWT_PUBLIC_KEY=LS0tLS1CRUdJTiBQVUJMSUMgS0VZLS0tLS0...
```

The values are base64-encoded PEM files (single line, no spaces) — safe to paste directly into `.env` files, Docker env vars, or Dokploy. Authstack decodes them at startup.

Keep `JWT_PRIVATE_KEY` secret. `JWT_PUBLIC_KEY` can be shared — consuming services use it to verify tokens locally or via `GET /.well-known/jwks.json`.

## Running the integration tests

### Full isolated run (recommended for CI)

```bash
make test-clean
```

This runs the complete lifecycle in one command:
1. Wipes any existing DB volume
2. Rebuilds and starts a fresh DB + API + test runner together
3. Stops everything as soon as the test container exits (`--abort-on-container-exit`)
4. Tears everything down, leaving no leftover state

### Run tests against already-running containers

```bash
make up
make test
```

## All commands

| Command | Description |
|---------|-------------|
| `make up` | Build images and start DB + API in the background |
| `make down` | Stop containers, keep DB volume |
| `make clean` | Stop containers and wipe the DB volume |
| `make test` | Run tests against already-running containers |
| `make test-clean` | Full isolated test run (wipe → start → test → teardown) |
| `make logs` | Tail API logs |
| `make db-shell` | Open a `psql` shell into the running database |
| `make keys` | Generate a fresh ES256 key pair |
| `make bootstrap` | Create the first instance admin via CLI (fresh database only) |

## Environment variables

| Variable | Required | Description |
|----------|----------|-------------|
| `DATABASE_URL` | Yes | Postgres connection string |
| `JWT_PRIVATE_KEY` | Yes | ES256 PEM private key (newlines as `\n`) |
| `JWT_PUBLIC_KEY` | Yes | ES256 PEM public key (newlines as `\n`) |
| `ACCESS_TOKEN_EXPIRY_SECS` | No | Access token lifetime in seconds (default: `900`) |
| `REFRESH_TOKEN_EXPIRY_SECS` | No | Refresh token lifetime in seconds (default: `2592000`) |
| `PORT` | No | HTTP port (default: `8080`) |
| `RUST_LOG` | No | Log filter, e.g. `authstack=debug` |

Bootstrap-only variables (not required for `authstack serve` after the first admin exists):

| Variable | Required | Description |
|----------|----------|-------------|
| `AUTHSTACK_BOOTSTRAP_EMAIL` | Bootstrap | Email for the first instance admin |
| `AUTHSTACK_BOOTSTRAP_PASSWORD` | Bootstrap | Password for the first instance admin |

## API reference

### Admin

Authstack ships with a browser-based admin panel at `/admin/login`. Log in with your admin credentials to manage applications.

#### Bootstrap

The first instance admin is created with the CLI — not over HTTP. This only works when the `admin_user` table is empty.

**Docker Compose** (default): set bootstrap variables in `.env`, then `make up`. The entrypoint runs bootstrap automatically on a fresh database:

```bash
AUTHSTACK_BOOTSTRAP_EMAIL=admin@example.com
AUTHSTACK_BOOTSTRAP_PASSWORD=your-strong-password
```

**Manual CLI** (local Rust or one-off container):

```bash
AUTHSTACK_BOOTSTRAP_EMAIL=admin@example.com \
AUTHSTACK_BOOTSTRAP_PASSWORD='your-strong-password' \
  cargo run -- bootstrap-admin
```

Or pipe the password:

```bash
echo 'your-strong-password' | cargo run -- bootstrap-admin \
  --email admin@example.com \
  --password-stdin
```

```text
created instance admin
id:    adm_...
email: admin@example.com
```

After that, open `http://localhost:8080/admin/login` in a browser to manage applications through the UI.

#### Admin panel routes

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET`  | `/admin/login` | — | Login page |
| `POST` | `/admin/login` | — | Submit login form → sets session cookie |
| `POST` | `/admin/logout` | Cookie | Clear session |
| `GET`  | `/admin/dashboard` | Cookie | List all applications |
| `GET`  | `/admin/apps/new` | Cookie | New application form |
| `POST` | `/admin/apps` | Cookie | Create application (form submit) |

#### JSON API (for scripts / CI)

After logging in and obtaining a session cookie, you can also use the JSON API:

```bash
# 1. Login and capture the session cookie
curl -c cookies.txt -X POST http://localhost:8080/admin/login \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "email=admin@example.com&password=your-password"

# 2. Create an application
curl -b cookies.txt -X POST http://localhost:8080/admin/applications \
  -H "Content-Type: application/json" \
  -d '{"name": "my-app"}'
```

Response (the `client_secret` is only returned once — store it securely):

```json
{
  "id": "app_01j4hz0r3fexwpbgm41z1w57at",
  "client_secret": "secret_xyz...",
  "name": "my-app"
}
```

### Auth

All `/auth/*`, `/users`, `/orgs`, and `/orgs/:id/members` endpoints require HTTP Basic auth:

```
Authorization: Basic base64(<app_id>:<client_secret>)
```

Where `<app_id>` is the TypeID returned when the application was created (e.g. `app_01j4hz0r3fexwpbgm41z1w57at`).

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/auth/signup` | Create a new user (auto-creates a personal org) |
| `POST` | `/auth/login` | Authenticate a user, returns access + refresh tokens |
| `POST` | `/auth/refresh` | Rotate refresh token, returns a new token pair |
| `POST` | `/auth/logout` | Revoke a refresh token |

**Signup**

```bash
curl -X POST http://localhost:8080/auth/signup \
  -H "Authorization: Basic $(echo -n 'app_01j4hz0r3fexwpbgm41z1w57at:secret_xyz' | base64)" \
  -H "Content-Type: application/json" \
  -d '{"name": "Alice", "email": "alice@example.com", "password": "hunter2secure"}'
```

**Login**

```bash
curl -X POST http://localhost:8080/auth/login \
  -H "Authorization: Basic $(echo -n 'app_01j4hz0r3fexwpbgm41z1w57at:secret_xyz' | base64)" \
  -H "Content-Type: application/json" \
  -d '{"email": "alice@example.com", "password": "hunter2secure"}'
```

```json
{
  "access_token": "<jwt>",
  "refresh_token": "<jwt>",
  "token_type": "Bearer"
}
```

The access token payload includes:

```json
{
  "sub": "usr_01j4hz0r3fexwpbgm41z1w57at",
  "app_id": "app_01j4hz0r3fexwpbgm41z1w57at",
  "org_id": "org_01j4hz0r3fexwpbgm41z1w57at",
  "org_type": "personal",
  "role": "owner",
  "email": "alice@example.com",
  "jti": "<uuid>",
  "iat": 1234567890,
  "exp": 1234568790
}
```

All entity IDs are TypeIDs — a prefixed, sortable identifier format. Prefixes: `app_` (application), `usr_` (user), `org_` (organization), `mbr_` (member), `acct_` (account), `rsess_` (refresh session), `adm_` (admin user).

### Users

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/users` | List all users in this application |
| `GET` | `/users/:id` | Get a user by ID |

### Organizations

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/orgs` | List all organizations in this application |
| `POST` | `/orgs` | Create a new team organization |
| `GET` | `/orgs/:id` | Get an organization by ID |

### Members

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/orgs/:id/members` | List members of an organization |
| `POST` | `/orgs/:id/members` | Add a user to an organization |
| `DELETE` | `/orgs/:id/members/:user_id` | Remove a user from an organization |

### JWKS

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/.well-known/jwks.json` | Public key set for JWT verification |

Consuming services can fetch this endpoint to verify Authstack-issued JWTs locally without making a round-trip for every request.

## Security model

- **App isolation:** Users, organizations, and credentials are scoped to an `app_id`. A `client_secret` is required to access any app's data — one app cannot read another app's users even if it knows the other app's TypeID.
- **Passwords:** Hashed with Argon2 (memory-hard, resistant to brute-force).
- **JWTs:** Signed with ES256 (asymmetric). Only Authstack holds the private key; consuming services verify with the public key.
- **Refresh token rotation:** Each use of a refresh token invalidates it and issues a new one. Re-use of a rotated token returns `401`.
- **Admin panel:** Protected by signed JWTs stored in `HttpOnly; SameSite=Strict` cookies. The first instance admin is created with `authstack bootstrap-admin` (CLI only).

## Local development (without Docker)

Requires Rust stable and a running PostgreSQL instance.

```bash
cp .env.example .env
# Fill in DATABASE_URL, JWT_PRIVATE_KEY, JWT_PUBLIC_KEY

# Create the first instance admin (fresh database only)
AUTHSTACK_BOOTSTRAP_EMAIL=admin@example.com \
AUTHSTACK_BOOTSTRAP_PASSWORD='your-password' \
  cargo run -- bootstrap-admin

cargo run       # runs migrations automatically, starts on :8080
cargo check     # fast type-check without full build
cargo build     # full build
```

Change all secrets in `.env` before deploying to production.
