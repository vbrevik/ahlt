# Enterprise Infrastructure Design

**Date:** 2026-02-21
**Status:** Approved
**Approach:** Incremental by layer (5 phases)

## Context

The im-ctrl application (crate `ahlt`) is a Rust/Actix-web system using SQLite with an EAV ontology pattern. Currently it runs locally on direct ports with no CI/CD or container orchestration. This design moves to a production-grade enterprise setup.

## Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Database | PostgreSQL 17 (master) + Neo4j Community (graph projection) | Multi-replica support, recursive CTEs, proper graph traversals |
| Kubernetes | Local-first via Rancher Desktop (k3s), cloud-ready later | Pragmatic — pipeline works locally, cloud migration is a swap of cluster credentials |
| CI/CD | Self-hosted GitLab CE on Mac M2 + GitLab CI | Full control, built-in registry, strong pipeline features |
| Routing | nginx Ingress with subdomain routing | dev.local / staging.local / app.local — clean, production-grade |
| Implementation order | Postgres first, then Compose, then GitLab, then K8s, then Ingress | Riskiest code change (Postgres) is isolated before any infra complexity |

## Phase 1a: PostgreSQL 17 Migration

**Goal:** Replace SQLite with PostgreSQL 17 while keeping the EAV schema pattern.

### Schema conversion

Core EAV tables map cleanly to Postgres:

```sql
entities        (id BIGINT GENERATED ALWAYS AS IDENTITY, entity_type TEXT, name TEXT, created_at TIMESTAMPTZ)
entity_properties (entity_id BIGINT REFERENCES entities(id) ON DELETE CASCADE, key TEXT, value TEXT)
relations       (id BIGINT GENERATED ALWAYS AS IDENTITY, relation_type_id BIGINT, source_id BIGINT, target_id BIGINT, created_at TIMESTAMPTZ)
relation_properties (relation_id BIGINT REFERENCES relations(id) ON DELETE CASCADE, key TEXT, value TEXT)
relation_types  (id BIGINT GENERATED ALWAYS AS IDENTITY, name TEXT UNIQUE)
```

Key dialect changes:
- `AUTOINCREMENT` → `GENERATED ALWAYS AS IDENTITY`
- `INTEGER` primary keys → `BIGINT`
- `DATETIME` → `TIMESTAMPTZ`
- SQLite pragmas removed (FK enforcement is default in Postgres, WAL is irrelevant)

### Rust driver migration

**Remove:** `rusqlite`, `r2d2`, `r2d2_sqlite`
**Add:** `sqlx` (features: `postgres`, `runtime-tokio`, `macros`)

Query pattern change:
```rust
// Before (rusqlite)
conn.query_row("SELECT ...", params![], |row| row.get(0))?

// After (sqlx)
sqlx::query_as!(Model, "SELECT ...", param).fetch_one(&pool).await?
```

Handler pattern change: `pool: web::Data<DbPool>` becomes `pool: web::Data<PgPool>`, async queries inline (no `web::block()` needed).

### PostgreSQL 17 features used

- `WITH RECURSIVE` for tree/graph traversals (permission chains, ancestry)
- `JSONB` for structured property values (action items, roll call — currently stored as JSON strings)
- `ON CONFLICT DO NOTHING` for idempotent seed operations (replaces `ConflictMode::Skip` logic)

### Migration tooling

`sqlx-cli` for creating and running migration files. `DATABASE_URL` env var controls target instance.

### Scope

~15 model files, all handlers, `db.rs`, `errors.rs` (`AppError::Db` variant type change), `main.rs` startup.

## Phase 1b: Neo4j Community Graph Projection

**Goal:** Offload graph traversals to Neo4j while Postgres stays the authoritative master.

### Architecture

```
Write path:  Handler → Postgres (source of truth) → async sync job → Neo4j
Read path:   Graph queries → Neo4j   |   All other queries → Postgres
```

### Data mapping

- Every `entity` → Neo4j node, labels from `entity_type`, properties from `entity_properties`
- Every `relation` → Neo4j relationship, type from `relation_types.name`, properties from `relation_properties`

### Queries that move to Neo4j (Cypher)

| Current pattern | Neo4j benefit |
|---|---|
| ABAC capability lookup (user → fills_position → tor_function → belongs_to_tor) | Variable-depth traversal, single Cypher query |
| Governance map rendering | Return full subgraph, not just pairs |
| Permission chain (user → role → permission) | Cleaner than recursive CTE |
| Warning context (who fills what, in which ToR) | Multi-hop without N+1 queries |

### Sync mechanism

A `graph_sync` Rust module — after any entity/relation create/update/delete in Postgres, fire an async Neo4j write (best-effort, non-blocking). If Neo4j is down, Postgres continues working; Neo4j self-heals on reconnect via a full-resync endpoint.

### Neo4j Community runs in Docker alongside the app.

## Phase 2: Docker Compose Multi-Environment

**Goal:** Run dev/staging/prod locally via Compose, establishing the container topology K8s will mirror.

### Service topology

```
┌─────────────────────────────────────────────────┐
│  Docker Compose                                 │
│                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│  │ app-dev  │  │ app-stg  │  │ app-prod │      │
│  │ :8080    │  │ :8081    │  │ :8082    │      │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘      │
│       │              │              │            │
│  ┌────▼──────────────▼──────────────▼─────┐     │
│  │  postgres:17  (ahlt_dev/stg/prod DBs)  │     │
│  └────────────────────────────────────────┘     │
│  ┌─────────────────────────────────────────┐    │
│  │  neo4j-community  :7474/:7687           │    │
│  └─────────────────────────────────────────┘    │
└─────────────────────────────────────────────────┘
```

Single Postgres container, three databases (`ahlt_dev`, `ahlt_staging`, `ahlt_prod`). One Neo4j instance shared across environments (namespaced by `env` property on nodes).

### File structure

```
docker-compose.yml          # base: postgres, neo4j, shared networks
docker-compose.dev.yml      # app-dev service + dev env vars
docker-compose.staging.yml  # app-staging service
docker-compose.prod.yml     # app-prod service
.env.dev / .env.staging / .env.prod   # secrets per env (gitignored)
Makefile                    # shortcuts: make dev, make staging, make prod, make all
```

### Environment variables

```env
# .env.dev example
APP_ENV=dev
PORT=8080
DATABASE_URL=postgresql://ahlt:secret@postgres:5432/ahlt_dev
NEO4J_URI=bolt://neo4j:7687
NEO4J_USER=neo4j
NEO4J_PASSWORD=secret
SESSION_KEY=<64+ char hex string>
COOKIE_SECURE=false
RUST_LOG=debug
```

Prod sets `COOKIE_SECURE=true`, `RUST_LOG=info`, strong `SESSION_KEY`.

## Phase 3: Self-hosted GitLab + CI Pipeline

**Goal:** Self-hosted git, container registry, and automated build/test/deploy pipeline on Mac M2.

### GitLab CE in Docker (ARM64)

| Service | Port |
|---|---|
| GitLab web UI | 8929 |
| GitLab SSH | 2222 |
| GitLab Container Registry | 5050 |

`/etc/hosts`:
```
127.0.0.1  gitlab.local registry.gitlab.local
```

Data volumes mounted to `~/gitlab-data/` for persistence.

### GitLab Runner

Runs as a Docker container registered against the local GitLab instance. Uses Docker executor with host Docker socket shared (no Docker-in-Docker needed).

### Repository migration

Import from GitHub via GitLab UI, then update local remote. GitHub can optionally stay as a read-only push mirror.

### CI Pipeline (`.gitlab-ci.yml`)

**Branch strategy:**
- Feature branches → `test` + `lint` only
- `main` → full pipeline + auto-deploy to staging
- Tags (`v*`) → full pipeline + manual-gate deploy to prod

**Stages:**

| Stage | What | Image |
|---|---|---|
| `test` | `cargo test --all` | `rust:1.84` |
| `lint` | `cargo clippy -- -D warnings` | `rust:1.84` |
| `build` | Docker build, push to `registry.gitlab.local:5050` | `docker:latest` |
| `deploy-staging` | Helm upgrade to k3s staging namespace | shell runner |
| `deploy-prod` | Same, `when: manual` | shell runner |

**Secrets:** `DATABASE_URL_*`, `SESSION_KEY_*`, `NEO4J_PASSWORD` stored as GitLab CI variables.

## Phase 4: Kubernetes with Rancher Desktop

**Goal:** Replace Docker Compose with k3s, Helm charts for repeatable deployments and rollbacks.

### Namespace strategy

```
ahlt-dev         ← dev app instance
ahlt-staging     ← staging app instance
ahlt-prod        ← prod app instance
shared-infra     ← Postgres 17 + Neo4j Community
```

### Helm chart structure

```
helm/
├── ahlt/                        # Application chart
│   ├── Chart.yaml
│   ├── templates/
│   │   ├── deployment.yaml
│   │   ├── service.yaml
│   │   ├── configmap.yaml
│   │   ├── secret.yaml
│   │   └── ingress.yaml
│   ├── values.yaml              # shared defaults
│   ├── values-dev.yaml          # 1 replica, RUST_LOG=debug
│   ├── values-staging.yaml      # 1 replica, RUST_LOG=info
│   └── values-prod.yaml         # 2 replicas, COOKIE_SECURE=true
├── infra/
│   ├── postgres.yaml            # Bitnami PostgreSQL chart or StatefulSet
│   └── neo4j.yaml               # Neo4j Community Helm chart
```

### K8s resources per environment

| Resource | Purpose |
|---|---|
| Deployment | App container, image from GitLab registry |
| Service | ClusterIP (internal only until Ingress) |
| ConfigMap | `APP_ENV`, `RUST_LOG`, `NEO4J_URI` |
| Secret | `DATABASE_URL`, `SESSION_KEY`, `NEO4J_PASSWORD` |
| PVC | Postgres data + Neo4j data (in shared-infra) |

### Registry access

k3s configured to trust local GitLab registry:
```yaml
# /etc/rancher/k3s/registries.yaml
mirrors:
  "registry.gitlab.local:5050":
    endpoint:
      - "http://registry.gitlab.local:5050"
```

### CI pipeline deploys via Helm

```yaml
deploy-staging:
  script:
    - helm upgrade --install ahlt-staging ./helm/ahlt
        -n ahlt-staging --create-namespace
        -f helm/ahlt/values-staging.yaml
        --set image.tag=$CI_COMMIT_SHA
```

## Phase 5: nginx Ingress + Subdomain Routing

**Goal:** Replace direct ports with clean subdomain-based routing through nginx.

### Setup

Disable k3s default Traefik, install nginx Ingress Controller:
```bash
helm install nginx-ingress ingress-nginx/ingress-nginx -n ingress-nginx --create-namespace
```

### Subdomain scheme

| Subdomain | Routes to | Namespace |
|---|---|---|
| `dev.local` | ahlt-dev Service | `ahlt-dev` |
| `staging.local` | ahlt-staging Service | `ahlt-staging` |
| `app.local` | ahlt-prod Service | `ahlt-prod` |
| `gitlab.local` | GitLab CE | `shared-infra` |

`/etc/hosts`:
```
127.0.0.1  dev.local staging.local app.local gitlab.local
```

### TLS with mkcert (local)

```bash
brew install mkcert
mkcert -install
mkcert dev.local staging.local app.local gitlab.local
kubectl create secret tls ahlt-dev-tls --cert=... --key=... -n ahlt-dev
```

Browsers trust mkcert certs without warnings. For cloud: replace with cert-manager + Let's Encrypt.

### Ingress Helm template

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: {{ .Release.Name }}
spec:
  ingressClassName: nginx
  tls:
    - hosts: [{{ .Values.ingress.host }}]
      secretName: {{ .Release.Name }}-tls
  rules:
    - host: {{ .Values.ingress.host }}
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: {{ .Release.Name }}
                port:
                  number: 8080
```

Values per environment:
```yaml
# values-dev.yaml       values-staging.yaml     values-prod.yaml
ingress:                 ingress:                ingress:
  host: dev.local          host: staging.local     host: app.local
```

All traffic enters through ports 80/443 via the Ingress Controller. Direct ports (8080/8081/8082) retired.

## Risk Summary

| Phase | Biggest risk | Mitigation |
|---|---|---|
| 1a PostgreSQL | ~15 model files rewritten, all handlers | Prompt-contracts per task, incremental migration |
| 1b Neo4j | Sync mechanism reliability | Best-effort sync, Postgres always works standalone |
| 2 Docker Compose | Port conflicts, M2 resource limits | Fixed port scheme, resource monitoring |
| 3 GitLab + CI | GitLab RAM usage (~4-6 GB) on M2 | Monitor allocations, 10-12 GB total for Docker |
| 4 Kubernetes | Registry trust, kubeconfig wiring | k3s registries.yaml, CI variable for kubeconfig |
| 5 nginx Ingress | mkcert trust chain | `mkcert -install` adds local CA to system trust |

## Implementation Notes

- Every implementation task will use **prompt-contracts** (GOAL / CONSTRAINTS / FORMAT / FAILURE CONDITIONS) before any code is written
- Each phase is independently testable and shippable
- Phase 1a (Postgres) is the highest-risk, highest-effort change and is deliberately isolated before any infrastructure work
