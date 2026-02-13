# Handler Boilerplate: Approaches

## Current State (after PageContext)

Every authenticated handler follows this shape:

```rust
pub async fn list(pool: web::Data<DbPool>, session: Session) -> impl Responder {
    if let Err(resp) = require_permission(&session, "users.list") {  // 1. permission
        return resp;
    }
    let conn = match pool.get() {                                     // 2. connection
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };
    let ctx = PageContext::build(&session, &conn, "/users");          // 3. page context
    let users = user::find_all_display(&conn).unwrap_or_default();    // 4. page data
    let tmpl = UserListTemplate { ctx, users };                       // 5. template
    match tmpl.render() {                                             // 6. render
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}
```

Steps 1-3 and 6 repeat in every GET handler. Steps 4-5 are page-specific. POST handlers that redirect (create, update, delete) share steps 1-2 but not 3 or 6.

The question: is there a clean way to reduce the repeated parts further?

---

## Approach A: Keep As-Is

**Do nothing.** The current code is ~15-20 lines per GET handler with clear, linear control flow. Each step is visible and debuggable. Adding more pages means copy-paste of the shell and filling in steps 4-5.

**Pros:**
- Zero abstraction overhead — anyone can read a handler and understand it
- Easy to customize per-handler (different permission, different path)
- No hidden magic; errors point to obvious lines
- Rust's type system already catches template field mismatches at compile time

**Cons:**
- 8-10 lines of boilerplate per GET handler
- `match tmpl.render()` repeated everywhere
- Pool error handling duplicated

**Verdict:** Likely the best choice until there are 10+ handler files. The boilerplate is mechanical but not harmful.

---

## Approach B: `render()` Helper Function

Extract just the render step into a helper:

```rust
// src/helpers.rs
pub fn render(tmpl: &impl Template) -> HttpResponse {
    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}
```

Handlers become:

```rust
let tmpl = UserListTemplate { ctx, users };
render(&tmpl)
```

**Pros:**
- Tiny change, 3 lines saved per handler
- No new concepts, just a function
- Easy to extend later (add cache headers, content-security-policy, etc.)

**Cons:**
- Marginal improvement — the real repetition is steps 1-3, not 6
- Adds an import

**Verdict:** Low-effort, low-risk. Worth doing when adding the first cache/CSP header to avoid updating every handler. The `render()` name is generic enough to last.

---

## Approach C: `get_conn()` Helper

The pool error handling is the most annoying boilerplate because the `match` prevents using `?`:

```rust
// src/db.rs
pub fn get_conn(pool: &DbPool) -> Result<r2d2::PooledConnection<...>, HttpResponse> {
    pool.get().map_err(|_| HttpResponse::InternalServerError().body("Database error"))
}
```

Handlers become:

```rust
let conn = get_conn(&pool)?;
```

Wait — this doesn't work because `impl Responder` isn't `Result`. We'd need to change the return type:

```rust
pub async fn list(...) -> Result<HttpResponse, HttpResponse> {
    let conn = get_conn(&pool)?;
    // ...
    Ok(render(&tmpl))
}
```

Actix supports `Result<HttpResponse, HttpResponse>` as a responder.

**Pros:**
- Eliminates the 3-line match on every handler
- `?` operator feels natural in Rust
- Composes with Approach B

**Cons:**
- Changes every handler's return type signature
- `Result<HttpResponse, HttpResponse>` looks odd — both arms are the same type
- `require_permission` would also need to return `Result<(), HttpResponse>` (it already does, but the `if let Err` pattern would become `require_permission(...)?`)
- All early returns change from `return HttpResponse::...` to `return Err(HttpResponse::...)`

**Verdict:** Makes the code more idiomatic Rust but changes the shape of every handler. Worth batching with another change (e.g., adding proper error types) rather than doing standalone.

---

## Approach D: Custom Error Type + `?` Everywhere

Define a proper error type that converts to HttpResponse:

```rust
pub enum AppError {
    Db(String),
    Template(String),
    NotFound(String),
    Forbidden,
    PasswordHash,
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::Db(_) => HttpResponse::InternalServerError().body("Database error"),
            AppError::Template(_) => HttpResponse::InternalServerError().body("Template error"),
            AppError::NotFound(msg) => HttpResponse::NotFound().body(msg.clone()),
            AppError::Forbidden => HttpResponse::SeeOther()
                .insert_header(("Location", "/dashboard"))
                .finish(),
            AppError::PasswordHash => HttpResponse::InternalServerError().body("Password hash error"),
        }
    }
}
```

Handlers become:

```rust
pub async fn list(pool: web::Data<DbPool>, session: Session) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;
    let conn = pool.get().map_err(|e| AppError::Db(e.to_string()))?;
    let ctx = PageContext::build(&session, &conn, "/users");
    let users = user::find_all_display(&conn).unwrap_or_default();
    let tmpl = UserListTemplate { ctx, users };
    Ok(render(&tmpl))
}
```

**Pros:**
- Most idiomatic Rust — `?` everywhere, clean error propagation
- Centralized error response formatting
- Easy to add logging, error pages, etc. in one place
- Handlers shrink to ~8 lines for simple GETs

**Cons:**
- AppError already exists in `src/errors.rs` but is unused — needs completing
- Requires `From` impls or `.map_err()` calls for each error source
- Form validation errors don't fit this pattern (they re-render the form, not a generic error page)
- POST handlers with "render form on error" still need the full match block

**Verdict:** The right long-term direction. The error type already exists (unused). Works best for GET handlers and redirect-on-error POSTs. Form re-rendering on validation errors stays as explicit match blocks — those are genuinely page-specific logic, not boilerplate.

---

## Approach E: Actix Middleware / Extractor

Create a custom extractor that bundles Session + Connection + PageContext:

```rust
pub struct Authenticated {
    pub session: Session,
    pub conn: PooledConnection<...>,
    pub ctx: PageContext,
}

impl FromRequest for Authenticated {
    // Extract session, get pool from app_data, get conn, build PageContext
}
```

Handler signature:

```rust
pub async fn list(auth: Authenticated) -> impl Responder {
    let users = user::find_all_display(&auth.conn).unwrap_or_default();
    let tmpl = UserListTemplate { ctx: auth.ctx, users };
    render(&tmpl)
}
```

**Pros:**
- Eliminates ALL boilerplate — handlers are pure page logic
- Permission check could be a separate extractor: `RequirePermission<"users.list">`
- Very clean handler signatures

**Cons:**
- `current_path` for PageContext isn't available in the extractor (Actix extractors don't know the semantic "page" path, only the raw request path) — would need `req.path()` which may not match the nav URL pattern (e.g., `/users/12/edit` vs `/users`)
- Permission code varies per handler — can't be in a generic extractor without const generics or runtime config
- Extractors that fail return generic errors, harder to customize
- Significant framework coupling — harder to understand for newcomers
- `from_fn` middleware has `'static` body constraints in Actix 4

**Verdict:** Over-engineered for current scale. The `current_path` problem is the deal-breaker — PageContext needs a page-specific path that the framework can't infer. A partial extractor (Session + Connection but not PageContext) might work but saves only 3 lines.

---

## Approach F: Declarative Macro

```rust
macro_rules! handler_get {
    ($fn_name:ident, $perm:expr, $path:expr, |$ctx:ident, $conn:ident| $body:block) => {
        pub async fn $fn_name(pool: web::Data<DbPool>, session: Session) -> impl Responder {
            if let Err(resp) = require_permission(&session, $perm) { return resp; }
            let $conn = match pool.get() {
                Ok(c) => c,
                Err(_) => return HttpResponse::InternalServerError().body("Database error"),
            };
            let $ctx = PageContext::build(&session, &$conn, $path);
            $body
        }
    };
}

// Usage:
handler_get!(list, "users.list", "/users", |ctx, conn| {
    let users = user::find_all_display(&conn).unwrap_or_default();
    render(&UserListTemplate { ctx, users })
});
```

**Pros:**
- Zero runtime overhead
- Eliminates all boilerplate from GET handlers
- Enforces consistent pattern

**Cons:**
- Macros are hard to debug (error messages point to macro expansion, not source)
- IDE support is poor (no autocomplete inside macro bodies)
- Doesn't help POST handlers (which have the most code)
- `edit_form` takes `web::Path<i64>` — macro needs variants for different parameter sets
- Inflexible — any handler that deviates needs to bypass the macro entirely

**Verdict:** Clever but fragile. Macros hide complexity rather than reducing it. Not worth it for 6 handlers.

---

## Recommendation

**Now:** Keep Approach A (do nothing). The PageContext refactor already eliminated the worst repetition. Each handler is 15-20 lines of clear, linear code. This is maintainable.

**When adding 3+ more handler files (roles, settings, etc.):** Adopt Approach B (render helper) + Approach D (AppError type). Together they reduce GET handlers to ~8 lines while keeping explicit control flow. The `AppError` type already has a skeleton in `src/errors.rs`.

**Implementation sequence when ready:**
1. Complete `AppError` in `src/errors.rs` with `ResponseError` impl
2. Add `From<r2d2::Error>` and `From<askama::Error>` impls
3. Make `require_permission` return `Result<(), AppError>`
4. Add `render()` helper in `src/helpers.rs`
5. Migrate one handler file at a time, starting with the simplest (dashboard)
6. Leave form-validation error paths as explicit matches — they're page-specific logic

**Skip:** Approaches E (extractor) and F (macro). They solve a problem we don't have at current scale and introduce complexity that makes the codebase harder to understand.
