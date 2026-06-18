.PHONY: up down clean test test-clean logs db-shell keys bootstrap css

# Build admin Tailwind/shadcn CSS (requires Node.js)
css:
	npm install && npm run build:css

# ── local dev ─────────────────────────────────────────────────────────────────

# Start DB + API in background (persistent volume)
up:
	docker compose up db api --build -d

# Stop all containers (keep DB volume)
down:
	docker compose down

# Stop and wipe the DB volume (full clean slate)
clean:
	docker compose down -v

# ── testing ───────────────────────────────────────────────────────────────────

# Run integration tests against already-running containers.
# Requires JWT_PRIVATE_KEY and JWT_PUBLIC_KEY to be exported in your shell,
# or a .env file present in this directory.
test:
	docker compose --profile test run --build --rm tests

# Full clean test run: generate fresh keys → wipe DB → build → run tests → tear down.
# No pre-existing environment variables required.
#
# Uses --abort-on-container-exit so the run terminates the moment the tests
# container (or the API, if it crashes) stops — prevents indefinite hanging.
test-clean:
	@openssl ecparam -name prime256v1 -genkey -noout -out /tmp/_authstack_test_ec.pem 2>/dev/null
	@openssl pkcs8 -topk8 -nocrypt -in /tmp/_authstack_test_ec.pem -out /tmp/_authstack_test.pem 2>/dev/null
	@printf 'JWT_PRIVATE_KEY=%s\nJWT_PUBLIC_KEY=%s\n' \
		"$$(base64 < /tmp/_authstack_test.pem | tr -d '\n')" \
		"$$(openssl ec -in /tmp/_authstack_test_ec.pem -pubout 2>/dev/null | base64 | tr -d '\n')" \
		> /tmp/_authstack_test.env
	docker compose --env-file /tmp/_authstack_test.env down -v
	docker compose --env-file /tmp/_authstack_test.env --profile test up --build \
		--abort-on-container-exit --exit-code-from tests; \
		STATUS=$$?; \
		docker compose --env-file /tmp/_authstack_test.env logs api; \
		docker compose --env-file /tmp/_authstack_test.env down -v; \
		rm -f /tmp/_authstack_test_ec.pem /tmp/_authstack_test.pem /tmp/_authstack_test.env; \
		exit $$STATUS

# ── utilities ─────────────────────────────────────────────────────────────────

# Tail API logs
logs:
	docker compose logs -f api

# Open a psql shell into the DB
db-shell:
	docker compose exec db psql -U postgres authstack

# Generate a fresh ES256 key pair — output is base64-encoded, safe for env vars and .env files.
# Example usage:  make keys >> .env
keys:
	@openssl ecparam -name prime256v1 -genkey -noout -out /tmp/authstack_ec.pem 2>/dev/null
	@openssl pkcs8 -topk8 -nocrypt -in /tmp/authstack_ec.pem -out /tmp/authstack_ec_pkcs8.pem 2>/dev/null
	@printf 'JWT_PRIVATE_KEY=%s\n' "$$(base64 < /tmp/authstack_ec_pkcs8.pem | tr -d '\n')"
	@printf 'JWT_PUBLIC_KEY=%s\n' "$$(openssl ec -in /tmp/authstack_ec.pem -pubout 2>/dev/null | base64 | tr -d '\n')"
	@rm /tmp/authstack_ec.pem /tmp/authstack_ec_pkcs8.pem

# Create the first instance admin on a fresh database.
# Usage: make bootstrap EMAIL=admin@example.com PASSWORD=secret
bootstrap:
	@test -n "$(EMAIL)" || (echo "Usage: make bootstrap EMAIL=admin@example.com PASSWORD=..." && exit 1)
	@test -n "$(PASSWORD)" || (echo "Usage: make bootstrap EMAIL=admin@example.com PASSWORD=..." && exit 1)
	docker compose run --rm \
		-e AUTHSTACK_BOOTSTRAP_EMAIL="$(EMAIL)" \
		-e AUTHSTACK_BOOTSTRAP_PASSWORD="$(PASSWORD)" \
		api bootstrap-admin
