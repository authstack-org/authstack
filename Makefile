.PHONY: up down clean test test-clean logs db-shell keys

# Start DB + API in background (persistent volume)
up:
	docker compose up db api --build -d

# Stop all containers (keep DB volume)
down:
	docker compose down

# Stop and wipe the DB volume (full clean slate)
clean:
	docker compose down -v

# Run integration tests against already-running containers
test:
	docker compose --profile test run --rm tests

# Full clean test run: wipe DB → fresh start → run tests → tear down
test-clean:
	docker compose down -v
	docker compose up db api --build -d --wait
	docker compose --profile test run --rm tests
	docker compose down -v

# Tail API logs
logs:
	docker compose logs -f api

# Open a psql shell into the DB
db-shell:
	docker compose exec db psql -U postgres aegis

# Generate a fresh ES256 key pair and print export commands.
# Paste the output into your .env or CI secrets.
keys:
	@openssl ecparam -name prime256v1 -genkey -noout -out /tmp/aegis_ec.pem
	@echo "JWT_PRIVATE_KEY=$$(cat /tmp/aegis_ec.pem | awk '{printf "%s\\n", $$0}')"
	@openssl ec -in /tmp/aegis_ec.pem -pubout -out /tmp/aegis_ec_pub.pem 2>/dev/null
	@echo "JWT_PUBLIC_KEY=$$(cat /tmp/aegis_ec_pub.pem | awk '{printf "%s\\n", $$0}')"
	@rm /tmp/aegis_ec.pem /tmp/aegis_ec_pub.pem
