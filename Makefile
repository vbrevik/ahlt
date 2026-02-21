# AHLT — Docker Compose Multi-Environment Management
# Usage: make dev | make staging | make prod | make all | make down

COMPOSE = docker compose -f docker-compose.yml

.PHONY: dev staging prod all infra down logs-dev logs-staging logs-prod ps build clean

# Start individual environments
dev: infra
	$(COMPOSE) -f docker-compose.dev.yml up -d --build app-dev
	@echo "Dev running at http://localhost:8080"

staging: infra
	$(COMPOSE) -f docker-compose.staging.yml up -d --build app-staging
	@echo "Staging running at http://localhost:8081"

prod: infra
	$(COMPOSE) -f docker-compose.prod.yml up -d --build app-prod
	@echo "Prod running at http://localhost:8082"

# Start all environments
all: infra
	$(COMPOSE) -f docker-compose.dev.yml -f docker-compose.staging.yml -f docker-compose.prod.yml up -d --build
	@echo "All environments running: dev=8080 staging=8081 prod=8082"

# Start shared infrastructure only (postgres + neo4j)
infra:
	$(COMPOSE) up -d postgres neo4j
	@echo "Waiting for Postgres to be healthy..."
	@until docker compose -f docker-compose.yml exec -T postgres pg_isready -U ahlt > /dev/null 2>&1; do sleep 1; done
	@echo "Infrastructure ready"

# Stop everything
down:
	$(COMPOSE) -f docker-compose.dev.yml -f docker-compose.staging.yml -f docker-compose.prod.yml down

# View logs
logs-dev:
	$(COMPOSE) -f docker-compose.dev.yml logs -f app-dev

logs-staging:
	$(COMPOSE) -f docker-compose.staging.yml logs -f app-staging

logs-prod:
	$(COMPOSE) -f docker-compose.prod.yml logs -f app-prod

# Show running services
ps:
	$(COMPOSE) -f docker-compose.dev.yml -f docker-compose.staging.yml -f docker-compose.prod.yml ps

# Rebuild app image only (no cache)
build:
	docker build --no-cache -t ahlt:latest .

# Remove all data volumes (destructive!)
clean:
	@echo "This will delete ALL data volumes (postgres, neo4j). Press Ctrl+C to cancel."
	@sleep 3
	$(COMPOSE) -f docker-compose.dev.yml -f docker-compose.staging.yml -f docker-compose.prod.yml down -v
