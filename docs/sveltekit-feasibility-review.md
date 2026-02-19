# SvelteKit Feasibility Review for im-ctrl

## Current Architecture Snapshot

| Dimension | Current State |
|-----------|--------------|
| **Backend** | Actix-web 4 (Rust) — 115 routes in `src/main.rs` |
| **Templates** | 44 Askama HTML templates (compile-time, server-rendered) |
| **CSS** | 3 hand-written CSS files (~4,000 lines in `static/css/style.css`) |
| **Client JS** | Zero JS build chain — 18 inline `<script>` blocks across 11 templates |
| **AJAX** | 6 templates use `fetch()` for dynamic data (calendar, graphs, data manager) |
| **JSON APIs** | 5 handler files serve JSON (calendar, governance graph, data manager, role builder, ontology) |
| **WebSocket** | Active — warning toast notifications via `actix-ws` |
| **Auth** | Cookie-based sessions (`actix-session`), CSRF tokens, IP rate limiting |
| **DB** | SQLite with embedded `schema.sql`, EAV ontology model |
| **Tests** | 52 integration tests against the Rust backend |
| **JS tooling** | None — no `package.json`, no npm, no bundler |

---

## Migration Options

### Option A: SvelteKit as full replacement (replaces Askama + Actix routing)

- Rewrite all 44 templates as Svelte components
- Actix-web becomes a pure JSON API (strip all HTML rendering, ~60 handlers affected)
- Move all routing logic from Rust to SvelteKit's file-based router
- Re-implement auth flow (session cookies → JWT or SvelteKit server hooks)
- Re-implement CSRF (SvelteKit has its own patterns)
- Port WebSocket client to Svelte stores
- Add Node.js runtime alongside Rust binary (or use SvelteKit adapter-static with API proxy)

### Option B: SvelteKit as frontend layer (Actix stays as API)

- Keep Actix-web as a pure REST API server
- SvelteKit runs separately, proxies to Actix in dev, builds to static/SSR in prod
- Two processes in development, two deployment targets
- All 44 templates rewritten as `.svelte` files
- Session management needs a bridge (shared cookie or token-based)

### Option C: Incremental — Svelte islands in existing Askama templates

- Keep the current server-rendered flow
- Add a JS build step (Vite) that compiles individual Svelte components
- Mount Svelte components into specific `<div>` targets in Askama templates
- Only interactive areas get Svelte (calendar, workflow builder, role builder, graphs)
- No SvelteKit routing — just Svelte as a component library

---

## Benefits

1. **Reactive UI for complex views** — The role builder wizard, workflow builder, calendar outlook, and ontology graph all have significant inline JS that would be cleaner as Svelte components with reactive state, two-way binding, and transitions.

2. **Component reusability** — Currently, shared UI patterns (cards, forms, tables, status badges) are copy-pasted across 44 templates. Svelte components would DRY this up significantly.

3. **Better developer experience for JS-heavy features** — Inline `<script>` blocks in Askama have no type checking, no imports, no linting. A proper Svelte setup gives you all of that.

4. **TypeScript support** — The current inline JS is untyped. SvelteKit projects get TypeScript out of the box with full IDE support.

5. **Client-side navigation** — SvelteKit's router would give instant page transitions without full-page reloads. Currently every link triggers a full server round-trip.

6. **Ecosystem access** — npm ecosystem for UI components, charting libraries (replace D3 inline), date pickers, etc.

7. **WebSocket integration** — Svelte stores integrate naturally with WebSocket connections (cleaner than the current inline toast JS).

---

## Drawbacks

1. **Massive rewrite scope** — 44 templates, 115 routes, ~4,000 lines of CSS, all Askama `PageContext`/`PermissionGroup`/etc. template structs. This is not incremental — it's a ground-up frontend rewrite taking weeks to months.

2. **Two runtime environments** — Currently the app is a single `cargo run`. Adding SvelteKit means Node.js in your toolchain, `npm install`, a JS build step, and either a proxy setup or two separate processes. Deployment complexity doubles.

3. **Auth/session complexity** — Current auth is tightly integrated: `require_permission()` runs in the handler before any HTML is generated. With SvelteKit, you need to either:
   - Duplicate permission checks on both sides (Rust API + SvelteKit hooks), or
   - Trust the frontend (bad), or
   - Send permissions as API data and enforce only server-side (requires restructuring every handler)

4. **CSRF rethink** — The CSRF flow is Askama-native (token injected server-side into forms). SvelteKit would need a different approach (double-submit cookie, custom headers).

5. **Askama compile-time safety lost** — Currently, if a template references a missing field, `cargo check` catches it. With SvelteKit, type errors in data fetching only surface at runtime (unless you invest heavily in TypeScript contracts between Rust API and Svelte).

6. **Performance regression for simple pages** — Pages like user lists, audit logs, and settings are pure CRUD that render instantly with server-side Askama. Adding a JS framework, hydration, and API calls will make these *slower*, not faster.

7. **Test coverage gap** — The 52 integration tests exercise handlers-that-return-HTML. Moving to an API + SvelteKit model means you need a whole new layer of frontend tests (Playwright, Vitest, etc.) while also rewriting the existing tests to target JSON responses.

8. **Single-developer velocity** — Adding a JS build chain, a second language runtime, and the SvelteKit learning curve will slow down feature delivery significantly during the transition.

9. **SQLite + single-process model breaks** — The current architecture benefits from single-process simplicity (one Rust binary, one SQLite file, no network hops). A SvelteKit frontend communicating over HTTP adds latency and failure modes.

---

## Conclusion

**Don't do it — not as a full migration.**

The current stack (Actix + Askama + inline JS + hand-written CSS) is well-suited to what im-ctrl actually is: a server-rendered CRUD application with a few interactive islands. The 80% of the UI that is forms, tables, and detail views gains nothing from SvelteKit and would be actively harder to maintain across two languages.

### Recommendation

1. **Option C (Svelte islands)** for the 3-4 genuinely interactive views. Add Vite + Svelte to compile specific components (workflow builder, calendar, role wizard, ontology graph). Mount them into Askama templates via `<div id="svelte-calendar"></div>`. This gives reactive UI where it matters without touching the 40 templates that work fine as server-rendered HTML.

2. **Or: keep improving the vanilla JS.** The inline scripts are already functional. The `el()` helper pattern, `fetch()` for APIs, and D3 for graphs cover current needs. The security hook enforcing `createElement` over `innerHTML` already pushes toward clean DOM manipulation.

The cost/benefit ratio of a full SvelteKit migration is very unfavorable for a project at this scale and stage. The 20% of views that need interactivity don't justify rewriting the other 80%.
