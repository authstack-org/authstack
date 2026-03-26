# Running Aegis in Production

This guide covers deploying Aegis on a Dokploy-managed server (the same setup used for brisk-api and brisk-auth). The steps apply equally to any Docker-capable host.

---

## Prerequisites

- A running PostgreSQL instance (Dokploy managed or external)
- Docker Hub credentials (to pull the image)
- A domain / subdomain pointing to your server (e.g. `auth.yourdomain.com`)

---

## 1. Docker image

The `docker-build-push` GitHub Actions workflow builds the image on every push to `main` and pushes it to Docker Hub tagged as:

```
<DOCKER_USERNAME>/<DOCKER_REPOSITORY>:aegis-latest
```

Set the following repository secrets in GitHub before the first push:

| Secret | Description |
|--------|-------------|
| `DOCKER_USERNAME` | Your Docker Hub username |
| `DOCKER_PASSWORD` | Your Docker Hub access token |
| `DOCKER_REPOSITORY` | Repository name (e.g. `brisk`) |

Once the workflow runs, the image is available to pull from any Docker host.

---

## 2. Generate a key pair

Run this once on any machine with `openssl` installed:

```bash
make keys
```

Copy the two output lines — you will paste them as environment variables in Dokploy.

> `JWT_PRIVATE_KEY` must be kept secret.
> `JWT_PUBLIC_KEY` can be distributed to consuming services that verify tokens locally.

---

## 3. Dokploy setup

### 3a. Create the service

1. In Dokploy, create a new **Application**.
2. Set the source to **Docker Image** and enter:
   ```
   <DOCKER_USERNAME>/<DOCKER_REPOSITORY>:aegis-latest
   ```
3. Set the internal port to `8080`.

### 3b. Environment variables

Add these in the Dokploy environment tab. All values are required unless marked optional.

| Variable | Example / Notes |
|----------|-----------------|
| `DATABASE_URL` | `postgres://user:pass@host:5432/aegis` |
| `JWT_PRIVATE_KEY` | Base64-encoded PKCS#8 PEM — output of `make keys` |
| `JWT_PUBLIC_KEY` | Base64-encoded SPKI PEM — output of `make keys` |
| `AEGIS_ADMIN_KEY` | A long random string, e.g. `openssl rand -hex 32` |
| `ACCESS_TOKEN_EXPIRY_SECS` | Optional. Default `900` (15 min) |
| `REFRESH_TOKEN_EXPIRY_SECS` | Optional. Default `2592000` (30 days) |
| `PORT` | Optional. Default `8080` |
| `RUST_LOG` | Optional. `aegis=info` for production |

> **Key format:** `make keys` outputs the PEM files base64-encoded as a single line with no spaces. Paste the value directly into Dokploy. Aegis decodes it at startup — no manual newline escaping needed.

### 3c. Health check

Configure Dokploy's health check to hit:

```
GET /.well-known/jwks.json
```

This returns `200` when the service is up and the JWT keys loaded correctly.

### 3d. Deploy

Click **Deploy** in Dokploy. On first boot, Aegis automatically runs all database migrations.

Check the container logs for:

```
aegis listening on 0.0.0.0:8080
```

If you see `failed to initialise JWT service`, the `JWT_PRIVATE_KEY` env var is missing or malformed — re-run `make keys` and paste the output into Dokploy.

---

## 4. Register your first application

Once Aegis is running, register each consuming app via the admin API:

```bash
curl -X POST https://auth.yourdomain.com/admin/applications \
  -H "Content-Type: application/json" \
  -H "X-Admin-Key: <AEGIS_ADMIN_KEY>" \
  -d '{"name": "my-app"}'
```

Response:

```json
{
  "id": "...",
  "client_id": "app_abc123",
  "client_secret": "secret_xyz...",
  "name": "my-app"
}
```

> **Store `client_secret` immediately** — it is hashed and cannot be retrieved again.

Give the `client_id` and `client_secret` to your app's BFF (backend-for-frontend). The BFF authenticates every Aegis request using HTTP Basic auth with those credentials.

---

## 5. Updating

Push to `main`. The `docker-build-push` workflow rebuilds and pushes `aegis-latest`. In Dokploy, trigger a redeploy (or enable auto-deploy on image update).

---

## 6. Security checklist

- [ ] `AEGIS_ADMIN_KEY` is at least 32 random bytes (`openssl rand -hex 32`)
- [ ] `JWT_PRIVATE_KEY` is stored only in Dokploy env vars — never committed to the repo
- [ ] PostgreSQL is not publicly accessible (use a private network or internal Dokploy network)
- [ ] HTTPS is terminated at the Dokploy reverse proxy (Traefik)
- [ ] `client_secret` values are stored in each BFF's secret store, not in client-side code
