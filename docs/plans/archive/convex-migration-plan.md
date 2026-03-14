# Migration Plan: im-ctrl → Convex + SvelteKit (Ontology-First)

## Executive Summary

Migrate im-ctrl from **Rust/Actix-web/PostgreSQL/Askama** to **Convex (self-hosted) + SvelteKit**, preserving the **ontology-first EAV architecture** — entities, properties, and relations remain the core data model, now stored as Convex documents instead of PostgreSQL rows.

**Key design decisions**:
1. **Ontology-first preserved**: 3 core tables (`entities`, `entityProperties`, `relations`) stay as the backbone. Pure EAV, no denormalization.
2. **SvelteKit frontend**: Replaces Askama server-rendered templates. Uses `convex-svelte` for reactive queries.
3. **Convex self-hosted**: Docker container running locally or on your own infrastructure. No cloud dependency.
4. **Incremental migration**: Prove viability with a vertical slice first, then expand feature by feature.

**Estimated scope**: ~140 routes, ~75 templates, 18 entity types, 35+ relation types, 44 permissions

---

## Stack Decisions

### Self-Hosted / Local-First Infrastructure

All components run locally or on your own servers. No cloud vendor lock-in.

| Component | Local Development | Self-Hosted Production |
|-----------|------------------|----------------------|
| **Convex backend** | Docker container on localhost:3210 | Docker on your server / VM / K8s |
| **Convex dashboard** | Docker on localhost:6791 | Same Docker compose |
| **Convex database** | SQLite (default, zero-config) | PostgreSQL (your own instance) |
| **SvelteKit frontend** | `npm run dev` on localhost:5173 | Node adapter → any server / Docker |
| **File storage** | Convex built-in (local volume) | Convex built-in (persistent volume) |

**Convex self-hosted setup**:
```bash
# Download docker-compose.yml from convex-backend repo
curl -O https://raw.githubusercontent.com/get-convex/convex-backend/main/self-hosted/docker-compose.yml
docker compose up -d

# Generate admin key
docker compose exec backend ./generate_admin_key.sh

# Configure project
# .env.local (not committed)
CONVEX_SELF_HOSTED_URL='http://127.0.0.1:3210'
CONVEX_SELF_HOSTED_ADMIN_KEY='<generated key>'
```

Backend on http://127.0.0.1:3210, dashboard on http://localhost:6791. Default storage is SQLite in a Docker volume — swap to PostgreSQL for production via config.

Sources:
- [Self-Hosting with Convex](https://stack.convex.dev/self-hosted-develop-and-deploy)
- [Convex Self-Hosting Docs](https://docs.convex.dev/self-hosting)
- [convex-backend self-hosted README](https://github.com/get-convex/convex-backend/blob/main/self-hosted/README.md)
- [Docker Compose Guide](https://www.bitdoze.com/convex-self-host/)

---

### Auth Provider: Arguments & Recommendation

Three viable options for SvelteKit + Convex. All store data in your Convex database (no external auth service dependency).

#### Option A: Convex Auth (Built-in)

| Dimension | Assessment |
|-----------|------------|
| **Setup complexity** | Low — 10 lines of config, works out of the box |
| **SvelteKit support** | Partial — primarily documented for React; Svelte requires manual token management via `useConvexClient().setAuth()` |
| **Features** | Email/password, OAuth (GitHub, Google, Apple), magic links/OTP |
| **Missing** | No MFA, no organization/team model, no admin dashboard, no pre-built UI |
| **Data storage** | In your Convex database (users table managed by Convex Auth) |
| **Self-hosted** | Yes — runs inside the Convex backend container |
| **Maturity** | Beta — documented as evolving; fewer SvelteKit examples |
| **Vendor coupling** | Tight — deeply tied to Convex's auth identity system |

**Verdict**: Simplest setup if you only need password + optional OAuth. Weakest SvelteKit story.

#### Option B: Better Auth + Convex Component

| Dimension | Assessment |
|-----------|------------|
| **Setup complexity** | Medium — ~6 files to configure, but well-documented for SvelteKit |
| **SvelteKit support** | Strong — dedicated SvelteKit adapter (`@mmailaender/convex-better-auth-svelte`), SSR hooks, cookie-based token extraction |
| **Features** | Email/password, OAuth, magic links, organizations, sessions, rate limiting, plugins ecosystem |
| **Missing** | MFA available via plugins; admin UI via community `convex-better-auth-ui` (shadcn-style, copy into project) |
| **Data storage** | In your Convex database via component adapter — no external service |
| **Self-hosted** | Fully — all auth data in your Convex, auth routes in your SvelteKit app |
| **Maturity** | Active development; Better Auth itself is well-established; Convex adapter is newer |
| **Vendor coupling** | Low — Better Auth is framework-agnostic; could swap Convex out later |

**Verdict**: Best fit for SvelteKit + self-hosted + feature-rich auth. More setup than Convex Auth but purpose-built SvelteKit integration.

#### Option C: Clerk

| Dimension | Assessment |
|-----------|------------|
| **Setup complexity** | Low — polished SDKs and pre-built UI components |
| **SvelteKit support** | Community SDK exists but not first-class |
| **Features** | Full: MFA, organizations, SSO, admin dashboard, user management portal |
| **Missing** | Nothing feature-wise — it's the most complete |
| **Data storage** | Clerk's cloud — auth data lives outside your infrastructure |
| **Self-hosted** | No — Clerk is a cloud service. Contradicts local-first requirement. |
| **Maturity** | Production-grade, battle-tested |
| **Vendor coupling** | High — Clerk controls your user data and auth flow |

**Verdict**: Eliminated. Cloud-only, can't self-host. Violates the local-first constraint.

#### Recommendation: Better Auth

**Better Auth + Convex component** is the strongest choice for this project because:
1. **SvelteKit-native**: Purpose-built adapter with SSR hooks, cookie auth, and type-safe client
2. **Self-hosted**: All auth data lives in your Convex database, no external service
3. **Feature-complete enough**: Email/password (what you have today), OAuth when needed, organizations for future ABAC expansion
4. **Low vendor coupling**: Better Auth is framework-agnostic; the Convex adapter is a thin layer
5. **Auth UI available**: Community `convex-better-auth-ui` provides shadcn-style login/signup/org components you copy into your project

Sources:
- [Convex + Better Auth](https://labs.convex.dev/better-auth)
- [SvelteKit Guide](https://labs.convex.dev/better-auth/framework-guides/sveltekit)
- [Better Auth Convex Integration](https://www.better-auth.com/docs/integrations/convex)
- [Convex Better Auth UI](https://etesie.dev/docs/auth/01-overview/01-introduction)
- [convex-better-auth-svelte npm](https://www.npmjs.com/package/@mmailaender/convex-better-auth-svelte)
- [Convex Auth Docs](https://docs.convex.dev/auth/convex-auth)
- [Convex Auth FAQ](https://labs.convex.dev/auth/faq)

---

### Svelte Component Library: Arguments & Recommendation

#### Option A: shadcn-svelte

| Dimension | Assessment |
|-----------|------------|
| **Philosophy** | Not a library — generates source code into your project. You own every component. |
| **Component count** | 60+ (Button, Card, Dialog, Data Table, Calendar, Chart, Sidebar, Form, etc.) |
| **Styling** | Tailwind CSS 4 |
| **Headless layer** | Built on Bits UI (which uses Melt UI internally) |
| **Accessibility** | Full ARIA, keyboard nav via Bits UI |
| **Svelte version** | Svelte 5 supported (migration guide available) |
| **Customization** | Total — source code lives in your repo, edit anything |
| **Ecosystem** | Largest Svelte component ecosystem; community ports of React shadcn patterns |
| **Data table** | Yes — built-in, with sorting, filtering, pagination |
| **Form handling** | Yes — integrates with Superforms + Zod validation |
| **AI-friendly** | Designed for code generation (components are plain Svelte files) |
| **Tradeoff** | You maintain the component code; updates require manual merge |

#### Option B: Skeleton UI

| Dimension | Assessment |
|-----------|------------|
| **Philosophy** | Traditional component library — install via npm, use as-is |
| **Component count** | 30+ (AppShell, DataTable, Modal, Drawer, etc.) |
| **Styling** | Tailwind CSS with design token system |
| **Headless layer** | None — opinionated rendering |
| **Accessibility** | Solid but less comprehensive than Bits UI |
| **Svelte version** | Svelte 5 support in v3 (newer, some rough edges) |
| **Customization** | Theme tokens + CSS overrides; can't change component internals easily |
| **Ecosystem** | Good but smaller than shadcn-svelte |
| **Data table** | Basic — less feature-rich than shadcn's |
| **Tradeoff** | Faster to start; harder to customize deeply; tied to their design system |

#### Option C: Bits UI (headless only)

| Dimension | Assessment |
|-----------|------------|
| **Philosophy** | Headless primitives — provides behavior, you provide all styling |
| **Component count** | 40+ accessible primitives |
| **Styling** | Bring your own (Tailwind, plain CSS, anything) |
| **Accessibility** | Excellent — core focus |
| **Svelte version** | Svelte 5 native |
| **Customization** | Maximum — you style everything from scratch |
| **Tradeoff** | Most work upfront; no pre-built visual design; essentially building your own component library |

#### Option D: Melt UI (headless, lower level)

| Dimension | Assessment |
|-----------|------------|
| **Philosophy** | Lowest-level headless builders — returns props/actions to spread onto your elements |
| **Styling** | Completely bring your own |
| **Svelte version** | Svelte 5 |
| **Tradeoff** | Maximum flexibility, maximum effort. Bits UI is built on top of Melt UI and is generally preferred unless you need the raw primitives. |

#### Recommendation: shadcn-svelte

**shadcn-svelte** is the strongest choice because:
1. **Owns the code**: Components live in your repo as plain Svelte files — no dependency risk, full control
2. **Comprehensive**: 60+ components covers everything this app needs (data tables, forms, dialogs, tabs, cards, calendar, charts)
3. **Data table built-in**: The current app's most complex UI pattern (users table with filtering/sorting/column picker/CSV export) has a direct shadcn equivalent
4. **Auth UI compatibility**: `convex-better-auth-ui` is built on shadcn-svelte — auth forms drop in seamlessly
5. **Tailwind CSS**: Aligns with the CSS migration strategy (replace 70+ BEM files with utility classes)
6. **AI-friendly**: Components are plain files — easy to generate, modify, extend with Claude
7. **Largest ecosystem**: Most tutorials, examples, and community patterns in the Svelte world

The tradeoff (maintaining component source) is acceptable because you already maintain 70+ CSS files and 75+ templates. Owning the component code gives you the same level of control you have today.

Sources:
- [shadcn-svelte Docs](https://www.shadcn-svelte.com/docs)
- [shadcn Component Comparison](https://github.com/jasongitmail/shadcn-compare)
- [Svelte UI Libraries Overview](https://joyofcode.xyz/svelte-ui-libraries)
- [SvelteKit UI Framework Discussion](https://github.com/sveltejs/kit/discussions/8389)
- [Svelte Library Comparison](https://svar.dev/blog/how-to-choose-svelte-library/)

---

## SvelteKit + Convex: Integration Details

The [`convex-svelte`](https://github.com/get-convex/convex-svelte) package (v0.0.12) provides:

| Feature | Support |
|---------|---------|
| Reactive queries (`useQuery`) | Yes — Svelte 5 runes, `isLoading`/`error`/`data` |
| Mutations & actions | Yes — via `useConvexClient()` → `client.mutation()` / `client.action()` |
| SSR (initial data) | Yes — `ConvexHttpClient` in `+page.server.ts` → `initialData` option |
| Auth token management | Yes — `client.setAuth()` via `useConvexClient()` |
| Type safety | Yes — full `api.*` generated types |
| Real-time subscriptions | Yes — same WebSocket reactivity as React client |

**Setup**:
```svelte
<!-- src/routes/+layout.svelte -->
<script lang="ts">
  import { PUBLIC_CONVEX_URL } from '$env/static/public';
  import { setupConvex } from 'convex-svelte';
  const { children } = $props();
  setupConvex(PUBLIC_CONVEX_URL);
</script>
{@render children()}
```

**Known limitation**: `convex-svelte` is less mature than `convex/react` — fewer community examples. The core reactivity is identical (same WebSocket protocol).

---

## Current System Inventory

| Dimension | Count |
|-----------|-------|
| HTTP routes | 140+ |
| HTML templates | 75+ |
| CSS files | 70+ |
| Entity types | 18 |
| Relation types | 35+ |
| Permissions | 44 |
| Workflow statuses | 13 (across 4 scopes) |
| Handler modules | 25+ |
| Model modules | 26+ |
| PostgreSQL tables | 4 core (EAV) + indexes |
| Background tasks | 4 scheduled generators |

---

## Phase 0: Project Scaffolding

### 0.1 Initialize SvelteKit + Convex (Self-Hosted)

```bash
# SvelteKit project
npx sv create im-ctrl-convex   # TypeScript, Tailwind CSS 4
cd im-ctrl-convex
npm install convex convex-svelte

# Self-hosted Convex backend
mkdir -p infra/convex
curl -o infra/convex/docker-compose.yml \
  https://raw.githubusercontent.com/get-convex/convex-backend/main/self-hosted/docker-compose.yml
cd infra/convex && docker compose up -d
docker compose exec backend ./generate_admin_key.sh

# Connect project to local backend
npx convex dev --url http://127.0.0.1:3210
```

### 0.2 Auth setup (Better Auth)

```bash
npm install @convex-dev/better-auth @mmailaender/convex-better-auth-svelte
npm install better-auth@1.4.9 --save-exact
```

### 0.3 Component library (shadcn-svelte)

```bash
npx shadcn-svelte@latest init
npx shadcn-svelte@latest add button card dialog data-table form input table tabs
```

### 0.4 Existing Rust app stays running
Both apps run simultaneously. Import seed data from PostgreSQL into Convex for development.

---

## Phase 1: Pure EAV Data Model

The current EAV pattern maps directly to Convex documents. **No denormalization, no typed tables per entity type**. The ontology is the schema.

### 1.1 Schema

```typescript
// convex/schema.ts
import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

export default defineSchema({
  // ═══════════════════════════════════════════════════
  //  CORE ONTOLOGY — the entire domain model
  // ═══════════════════════════════════════════════════

  entities: defineTable({
    entityType: v.string(),          // "user", "tor", "meeting", "proposal", etc.
    name: v.string(),                // Unique within type
    label: v.string(),               // Human-readable display name
    isActive: v.boolean(),
    sortOrder: v.number(),
  })
    .index("by_type", ["entityType"])
    .index("by_type_name", ["entityType", "name"])
    .index("by_type_active", ["entityType", "isActive"]),

  entityProperties: defineTable({
    entityId: v.id("entities"),
    key: v.string(),
    value: v.string(),
  })
    .index("by_entity", ["entityId"])
    .index("by_entity_key", ["entityId", "key"])
    .index("by_key_value", ["key", "value"]),

  relations: defineTable({
    relationType: v.string(),
    sourceId: v.id("entities"),
    targetId: v.id("entities"),
  })
    .index("by_source", ["sourceId"])
    .index("by_target", ["targetId"])
    .index("by_type", ["relationType"])
    .index("by_type_source", ["relationType", "sourceId"])
    .index("by_type_target", ["relationType", "targetId"])
    .index("by_source_type", ["sourceId", "relationType"])
    .index("by_target_type", ["targetId", "relationType"])
    .index("by_all", ["relationType", "sourceId", "targetId"]),

  relationProperties: defineTable({
    relationId: v.id("relations"),
    key: v.string(),
    value: v.string(),
  })
    .index("by_relation", ["relationId"])
    .index("by_relation_key", ["relationId", "key"]),

  // ═══════════════════════════════════════════════════
  //  SUPPORTING (outside ontology, high-volume/special)
  // ═══════════════════════════════════════════════════

  auditLog: defineTable({
    userId: v.optional(v.id("entities")),
    username: v.string(),
    action: v.string(),
    targetType: v.string(),
    targetId: v.optional(v.id("entities")),
    details: v.optional(v.any()),
  })
    .index("by_action", ["action"])
    .index("by_target_type", ["targetType"])
    .index("by_user", ["userId"]),

  files: defineTable({
    entityId: v.id("entities"),
    storageId: v.id("_storage"),
    filename: v.string(),
    mimeType: v.string(),
  })
    .index("by_entity", ["entityId"]),
});
```

**6 tables total** (4 ontology + audit + files). The entire domain model — users, roles, permissions, ToRs, meetings, proposals, workflows, warnings — lives in the 4 ontology tables, same as today.

### 1.2 Core Ontology Helpers

```typescript
// convex/lib/ontology.ts — the foundation everything builds on

// Get entity with all properties as { ...entity, properties: Record<string, string> }
export async function getEntityWithProps(ctx: QueryCtx, entityId: Id<"entities">)

// List entities of a type with properties
export async function listEntitiesOfType(ctx: QueryCtx, entityType: string)

// Find entity by type + name (unique)
export async function findEntityByName(ctx: QueryCtx, entityType: string, name: string)

// Follow relation forward: source → targets
export async function getRelated(ctx: QueryCtx, sourceId: Id<"entities">, relationType: string)

// Follow relation backward: target ← sources
export async function getReverseRelated(ctx: QueryCtx, targetId: Id<"entities">, relationType: string)

// Set/update a property (upsert)
export async function setProperty(ctx: MutationCtx, entityId: Id<"entities">, key: string, value: string)

// Create relation (idempotent, checks uniqueness)
export async function createRelation(ctx: MutationCtx, relationType: string, sourceId: Id<"entities">, targetId: Id<"entities">)

// Cascade delete: entity + its properties + all relations in both directions
export async function deleteEntity(ctx: MutationCtx, entityId: Id<"entities">)

// Find entities by property value (e.g. all proposals where status=draft)
export async function findByProperty(ctx: QueryCtx, key: string, value: string, entityType?: string)
```

These ~10 functions are the entire data access layer. Every domain module is a thin wrapper.

### 1.3 Data Migration

Same JSON-LD format the existing `/api/data/export` produces:
```bash
# Export from running Rust app
curl http://localhost:8080/api/data/export > seed-data.json

# Import into Convex (via action that batches inserts)
npx convex run dataManager:import --args '{"url": "file://seed-data.json"}'
```

PostgreSQL BIGINT IDs → Convex `_id` mapping handled during import. Relations re-linked by entity type+name lookup.

---

## Phase 2: Authentication & Authorization

### 2.1 Better Auth Setup (SvelteKit)

Following the [SvelteKit guide](https://labs.convex.dev/better-auth/framework-guides/sveltekit):

```typescript
// src/convex/auth.ts — Better Auth with Convex adapter
import { betterAuth } from "better-auth/minimal";
import { convex } from "@convex-dev/better-auth/plugins";

export const createAuth = (ctx) => betterAuth({
  baseURL: process.env.SITE_URL,
  database: authComponent.adapter(ctx),
  emailAndPassword: { enabled: true },
  plugins: [convex({ authConfig })],
});
```

```typescript
// src/hooks.server.ts — SvelteKit auth hook
import { getToken } from '@mmailaender/convex-better-auth-svelte/sveltekit';

export const handle = async ({ event, resolve }) => {
  event.locals.token = await getToken(createAuth, event.cookies);
  return resolve(event);
};
```

### 2.2 Bridge Auth to Ontology

Better Auth manages its own user records. Bridge to ontology user entities:

```typescript
// convex/lib/auth.ts
export async function getCurrentUserEntity(ctx: QueryCtx) {
  const identity = await ctx.auth.getUserIdentity();
  if (!identity) return null;

  // Find user entity linked to this auth identity via token_identifier property
  const tokenProp = await ctx.db
    .query("entityProperties")
    .withIndex("by_key_value", q =>
      q.eq("key", "token_identifier").eq("value", identity.tokenIdentifier))
    .unique();
  if (!tokenProp) return null;

  return getEntityWithProps(ctx, tokenProp.entityId);
}
```

### 2.3 Permission System (Ontology-Native)

Same graph traversal as today: `user → has_role → role → has_permission → permission`

```typescript
// convex/lib/authorization.ts
export async function getUserPermissions(ctx: QueryCtx, userId: Id<"entities">): Promise<Set<string>> {
  const roles = await getRelated(ctx, userId, "has_role");
  const permissions = new Set<string>();
  for (const role of roles) {
    const perms = await getRelated(ctx, role._id, "has_permission");
    for (const perm of perms) permissions.add(perm.name);
  }
  return permissions;
}

export async function requirePermission(ctx: QueryCtx, userId: Id<"entities">, code: string) {
  const perms = await getUserPermissions(ctx, userId);
  if (!perms.has(code)) throw new Error(`Permission denied: ${code}`);
}
```

### 2.4 ABAC (ToR Capabilities)

Same: `user → fills_position → tor_function → belongs_to_tor → tor`

```typescript
// convex/lib/abac.ts
export async function getTorCapabilities(ctx: QueryCtx, userId: Id<"entities">, torId: Id<"entities">): Promise<Set<string>> {
  const functions = await getRelated(ctx, userId, "fills_position");
  const capabilities = new Set<string>();
  for (const fn of functions) {
    const tors = await getRelated(ctx, fn._id, "belongs_to_tor");
    if (tors.some(t => t._id === torId)) {
      const caps = fn.properties.capabilities;
      if (caps) for (const c of caps.split(",")) capabilities.add(c.trim());
    }
  }
  return capabilities;
}
```

---

## Phase 3: Backend Functions

### 3.1 Structure

```
convex/
├── schema.ts
├── auth.ts                      # Better Auth config
├── http.ts                      # HTTP routes (auth, CSV export)
├── crons.ts                     # Warning generators
├── lib/
│   ├── ontology.ts              # Core EAV helpers
│   ├── auth.ts                  # User identity bridge
│   ├── authorization.ts         # RBAC
│   ├── abac.ts                  # ToR capabilities
│   ├── workflow.ts              # Transition engine
│   └── validators.ts            # Input validation
├── entities.ts                  # Generic CRUD
├── properties.ts                # Property CRUD
├── relations.ts                 # Relation CRUD
├── users.ts                     # User-domain wrapper
├── roles.ts                     # Role management
├── tors.ts                      # ToR domain
├── meetings.ts                  # Meeting domain + workflow
├── agendaPoints.ts              # Agenda
├── proposals.ts                 # Proposal workflow
├── suggestions.ts               # Suggestion workflow
├── minutes.ts                   # Minutes generation
├── documents.ts                 # Documents + file storage
├── workflow.ts                  # Workflow config CRUD
├── warnings.ts                  # Warning system + crons
├── audit.ts                     # Audit log
├── dashboard.ts                 # Dashboard aggregations
├── governance.ts                # Governance map data
├── dataManager.ts               # Import/export
└── node/
    └── csvExport.ts             # CSV generation (Node.js action)
```

### 3.2 Domain Modules Are Thin Wrappers

Each domain module adds business logic on top of generic ontology ops:

```typescript
// convex/tors.ts — example domain wrapper
export const list = query({
  args: {},
  handler: async (ctx, args) => {
    const user = await getCurrentUserEntity(ctx);
    if (!user) throw new Error("Not authenticated");
    await requirePermission(ctx, user._id, "tor.list");
    return listEntitiesOfType(ctx, "tor");
  },
});

export const getDetail = query({
  args: { id: v.id("entities") },
  handler: async (ctx, args) => {
    const tor = await getEntityWithProps(ctx, args.id);
    if (!tor || tor.entityType !== "tor") throw new Error("Not found");

    const functions = await getReverseRelated(ctx, args.id, "belongs_to_tor");
    const members = []; // for each function, get fills_position reverse
    const protocolSteps = await getReverseRelated(ctx, args.id, "protocol_of");
    const dependencies = await getRelated(ctx, args.id, "depends_on");
    const meetings = await getReverseRelated(ctx, args.id, "scheduled_for_meeting");

    return { tor, functions, members, protocolSteps, dependencies, meetings };
  },
});
```

### 3.3 Scheduled Functions

```typescript
// convex/crons.ts
import { cronJobs } from "convex/server";
import { internal } from "./_generated/api";

const crons = cronJobs();
crons.interval("check users without role", { minutes: 5 }, internal.warnings.checkUsersWithoutRole);
crons.interval("check tor vacancies", { minutes: 5 }, internal.warnings.checkTorVacancies);
crons.interval("cleanup old warnings", { hours: 1 }, internal.warnings.cleanupOldWarnings);
export default crons;
```

---

## Phase 4: Frontend (SvelteKit + shadcn-svelte)

### 4.1 Tech Stack

| Current | New |
|---------|-----|
| Askama templates | SvelteKit (SSR + client) |
| 70+ BEM CSS files | Tailwind CSS 4 via shadcn-svelte |
| Vanilla JS + D3.js | Svelte 5 + D3.js |
| No component lib | shadcn-svelte (60+ components) |

### 4.2 Route Structure

```
src/routes/
├── +layout.svelte              # setupConvex, auth, nav, sidebar
├── +page.svelte                # → /dashboard redirect
├── login/+page.svelte
├── dashboard/+page.svelte
├── users/
│   ├── +page.svelte            # DataTable + FilterBuilder
│   ├── new/+page.svelte
│   └── [id]/edit/+page.svelte
├── roles/
│   ├── +page.svelte            # Assignment matrix
│   └── builder/...
├── tor/
│   ├── +page.svelte
│   ├── new/+page.svelte
│   ├── outlook/+page.svelte
│   └── [id]/
│       ├── +page.svelte        # Detail tabs
│       ├── edit/+page.svelte
│       ├── proposals/...
│       ├── suggestions/...
│       ├── meetings/[mid]/+page.svelte
│       └── workflow/...
├── meetings/+page.svelte
├── minutes/[id]/+page.svelte
├── workflow/...
├── documents/...
├── warnings/...
├── governance/map/+page.svelte
├── ontology/...
├── audit/+page.svelte
├── account/+page.svelte
├── settings/+page.svelte
└── admin/data-manager/+page.svelte
```

### 4.3 SSR Pattern

```typescript
// src/routes/tor/+page.server.ts
import { ConvexHttpClient } from "convex/browser";
import { api } from "$convex/_generated/api";
import { PUBLIC_CONVEX_URL } from "$env/static/public";

export async function load() {
  const client = new ConvexHttpClient(PUBLIC_CONVEX_URL);
  return { tors: await client.query(api.tors.list, {}) };
}
```

```svelte
<!-- src/routes/tor/+page.svelte -->
<script lang="ts">
  import { useQuery } from 'convex-svelte';
  import { api } from '$convex/_generated/api';
  let { data } = $props();
  const tors = useQuery(api.tors.list, {}, { initialData: data.tors });
</script>

<!-- SSR renders immediately, then auto-updates reactively -->
```

### 4.4 D3.js Graphs in Svelte

```svelte
<!-- src/lib/components/OntologyGraph.svelte -->
<script lang="ts">
  import { useQuery } from 'convex-svelte';
  import { api } from '$convex/_generated/api';
  import * as d3 from 'd3';

  let svgEl: SVGSVGElement;
  const graphData = useQuery(api.governance.graphData, {});

  $effect(() => {
    if (graphData.data && svgEl) renderGraph(svgEl, graphData.data);
  });
</script>

<svg bind:this={svgEl} class="w-full h-full" />
```

---

## Phase 5: Incremental Migration Strategy

**Do not** rewrite everything at once. Prove viability with a vertical slice, then expand.

### Milestone 0: Proof of Concept (Week 1)

Prove the stack works end-to-end with the simplest possible vertical slice:

- [ ] Self-hosted Convex running in Docker
- [ ] SvelteKit project connected to local Convex
- [ ] Better Auth login/logout working
- [ ] Schema with 4 ontology tables deployed
- [ ] `lib/ontology.ts` core helpers (getEntityWithProps, listEntitiesOfType, setProperty, createRelation, deleteEntity)
- [ ] Import a subset of seed data (users, roles, permissions, 1 ToR)
- [ ] One working page: `/users` list with data from Convex
- [ ] Reactive: create a user in Convex dashboard → list updates live

**Gate**: If this milestone feels wrong, stop and reconsider before investing more.

### Milestone 1: Auth + User Management (Weeks 2-3)

Vertical slice: complete user lifecycle.

- [ ] Better Auth email/password login
- [ ] Auth → ontology user entity bridge
- [ ] Permission system (RBAC via ontology traversal)
- [ ] Users CRUD: list (with DataTable, sort, filter, search), create, edit, delete
- [ ] Roles: assignment matrix, builder
- [ ] Navigation sidebar (from ontology nav_item entities)
- [ ] Audit logging for user mutations
- [ ] Settings page

**Gate**: Can you log in, manage users/roles, see audit trail? Compare UX to the Rust app.

### Milestone 2: ToR Core (Weeks 4-6)

The most complex domain. If the EAV approach holds here, it holds everywhere.

- [ ] ToR CRUD (list, create, edit, delete)
- [ ] ToR detail page (tabs: info, positions/functions, protocol, dependencies, meetings)
- [ ] ToR member management (fills_position relations)
- [ ] ABAC capability checks
- [ ] Protocol steps (ordered entities)
- [ ] Dependencies (tor-to-tor relations)
- [ ] ToR context bar component

**Gate**: Can you fully manage a ToR and its members? Is the ontology traversal fast enough?

### Milestone 3: Workflow + Proposals/Suggestions (Weeks 7-8)

- [ ] Workflow engine (status entities, transition entities, transition validation)
- [ ] Workflow builder UI
- [ ] Proposals: create, edit, submit, review, approve/reject
- [ ] Suggestions: create, accept, reject
- [ ] Queue management

### Milestone 4: Meetings & Governance (Weeks 9-11)

- [ ] Meetings: create, confirm, start, complete, cancel
- [ ] Agenda points + CoA + opinions + decisions
- [ ] Minutes generation + editor
- [ ] Governance map (D3.js graph)
- [ ] ToR outlook calendar
- [ ] Warning system + cron jobs

### Milestone 5: Remaining Features (Weeks 12-13)

- [ ] Documents + file storage
- [ ] Presentation templates
- [ ] Data manager (import/export)
- [ ] Dashboard
- [ ] Ontology browser + graph
- [ ] CSV export
- [ ] Account management

### Milestone 6: Production Readiness (Week 14)

- [ ] Full data migration from PostgreSQL
- [ ] E2E tests (Playwright)
- [ ] Performance review (query times, scan limits)
- [ ] Self-hosted production Docker Compose (Convex + PostgreSQL + SvelteKit Node adapter)
- [ ] Backup strategy

### Each Milestone Includes

1. **Backend functions** for the domain
2. **Frontend pages** with SSR + reactive queries
3. **Tests** (convex-test for backend, Playwright for key flows)
4. **Side-by-side comparison** with the Rust app (same seed data)

If any milestone reveals that the approach doesn't work (performance, complexity, DX), you have a clear stop point with working code up to that point.

---

## Phase 6: Testing

### Backend (convex-test)
```typescript
import { convexTest } from "convex-test";
import schema from "../schema";

test("create entity and traverse relations", async () => {
  const t = convexTest(schema);
  const userId = await t.mutation(api.entities.create, {
    entityType: "user", name: "alice", label: "Alice",
  });
  const roleId = await t.mutation(api.entities.create, {
    entityType: "role", name: "admin", label: "Admin",
  });
  await t.mutation(api.relations.create, {
    relationType: "has_role", sourceId: userId, targetId: roleId,
  });
  const roles = await t.query(api.entities.getRelated, {
    sourceId: userId, relationType: "has_role",
  });
  expect(roles).toHaveLength(1);
  expect(roles[0].name).toBe("admin");
});
```

### Frontend (Playwright)
Port existing tests from `scripts/users-table.test.mjs`.

### Test Priority
1. Ontology helpers (CRUD + cascade + traversal)
2. Permission graph traversal
3. Workflow transitions
4. Domain modules (users, tors)
5. Data import/export round-trip
6. E2E login → CRUD → verify

---

## Phase 7: Self-Hosted Production Deployment

```yaml
# docker-compose.prod.yml
services:
  convex-backend:
    image: ghcr.io/get-convex/convex-self-hosted:latest
    ports:
      - "3210:3210"   # Backend API
      - "3211:3211"   # HTTP actions
    environment:
      - DATABASE_URL=postgresql://convex:password@postgres:5432/convex
    volumes:
      - convex-data:/data
    depends_on:
      - postgres

  convex-dashboard:
    image: ghcr.io/get-convex/convex-dashboard:latest
    ports:
      - "6791:6791"
    environment:
      - NEXT_PUBLIC_DEPLOYMENT_URL=http://convex-backend:3210

  postgres:
    image: postgres:17
    environment:
      - POSTGRES_DB=convex
      - POSTGRES_USER=convex
      - POSTGRES_PASSWORD=password
    volumes:
      - postgres-data:/var/lib/postgresql/data

  app:
    build: .  # SvelteKit with Node adapter
    ports:
      - "3000:3000"
    environment:
      - PUBLIC_CONVEX_URL=http://convex-backend:3210
      - ORIGIN=https://your-domain.com

volumes:
  convex-data:
  postgres-data:
```

Everything runs on a single server or VM. No cloud dependencies.

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| N+1 property reads per entity | Slow list queries | Convex reads co-located (~1ms); paginate; the incremental approach reveals this early at Milestone 0 |
| 1-second query/mutation limit | Complex operations timeout | Split into batched mutations; use actions for heavy processing |
| 32k document scan limit | Large entity lists | Paginate; `by_type` index scopes scans per entity type |
| No UNIQUE constraint | Duplicate entities | Check-then-insert in mutations (serialized transactions, safe) |
| No CASCADE DELETE | Orphaned properties/relations | `deleteEntity()` helper handles cascade |
| `convex-svelte` less mature | Edge cases | Core API is solid; fall back to `ConvexClient` directly |
| All property values are strings | Type coercion | Same as current PostgreSQL EAV; parse in helpers |
| No SQL aggregations | Dashboard/report queries | Compute in JS after collect; acceptable for current data volumes |
| Self-hosted Convex stability | New deployment model | SQLite for dev, PostgreSQL for prod; Docker volumes for persistence |
| Better Auth + Convex maturity | Auth edge cases | Active community, SvelteKit-specific adapter; graceful degradation |

---

## Resolved Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Infrastructure** | Self-hosted (Docker) | Runs locally, no cloud dependency |
| **Convex database** | SQLite (dev) / PostgreSQL (prod) | Zero-config locally, reliable in production |
| **EAV strategy** | Pure ontology, no denormalization | Preserves existing architecture; optimize later if needed |
| **Migration approach** | Incremental with gates | Prove viability before committing; stop points at each milestone |
| **Neo4j** | Dropped | Already optional; ontology traversal via Convex sufficient |
| **Frontend framework** | SvelteKit | User preference; good Convex integration via convex-svelte |
| **CSS** | Tailwind CSS 4 via shadcn-svelte | Replaces 70+ BEM files; components include styling |

## Open Decisions

| Decision | Options | Notes |
|----------|---------|-------|
| **Auth provider** | Better Auth (recommended) vs Convex Auth | See arguments above; decide before Milestone 1 |
| **Component library** | shadcn-svelte (recommended) vs Skeleton UI | See arguments above; decide before Milestone 0 |

---

## Sources

- [Convex Self-Hosting](https://docs.convex.dev/self-hosting) · [Setup Guide](https://stack.convex.dev/self-hosted-develop-and-deploy) · [Docker Guide](https://www.bitdoze.com/convex-self-host/)
- [convex-svelte](https://github.com/get-convex/convex-svelte) · [npm](https://www.npmjs.com/package/convex-svelte) · [Svelte Quickstart](https://docs.convex.dev/quickstart/svelte)
- [Convex + Better Auth](https://labs.convex.dev/better-auth) · [SvelteKit Guide](https://labs.convex.dev/better-auth/framework-guides/sveltekit) · [Better Auth Convex Plugin](https://www.better-auth.com/docs/integrations/convex)
- [convex-better-auth-svelte](https://www.npmjs.com/package/@mmailaender/convex-better-auth-svelte) · [Auth UI](https://etesie.dev/docs/auth/01-overview/01-introduction)
- [Convex Auth](https://docs.convex.dev/auth/convex-auth) · [Auth FAQ](https://labs.convex.dev/auth/faq)
- [Convex & Clerk](https://docs.convex.dev/auth/clerk)
- [shadcn-svelte](https://www.shadcn-svelte.com/docs) · [Component Comparison](https://github.com/jasongitmail/shadcn-compare)
- [Svelte UI Libraries](https://joyofcode.xyz/svelte-ui-libraries) · [SvelteKit UI Discussion](https://github.com/sveltejs/kit/discussions/8389)
- [SvelteKit & Convex Blog](https://www.modlguessr.com/blog/2025-09-21_sveltekit-x-convex)
- [Convex Schemas](https://docs.convex.dev/database/schemas) · [Limits](https://docs.convex.dev/production/state/limits)
