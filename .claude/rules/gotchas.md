# Gotchas & Quirks

## Askama 0.14

- **No `ref` in `if let`**: Use `{% if let Some(x) = val %}` not `{% if let Some(ref x) = val %}`
- **Included templates share parent scope**: Every template struct must carry fields used by included partials (e.g. `username`, `permissions` for nav)
- **String equality in loops**: Use `.as_str()` on both sides: `{% if field.as_str() == item.as_str() %}`
- **Can't call `Vec<String>::contains()` with `&str`**: Create wrapper types with template-friendly methods (e.g. `Permissions::has(&str)`)
- **No `&&` in `{% if %}` conditions**: Use nested `{% if a %}{% if b %}...{% endif %}{% endif %}` instead
- **No `||` in `{% if %}` conditions**: Use `{% if a %}...{% else %}{% if b %}...{% endif %}{% endif %}` — duplicate the inner block in both branches
- **No array indexing `arr[i]`**: Can't write `steps[idx-1]`. Find target by ID server-side, determine neighbour, then swap.
- **Partial files inherit parent struct**: When extracting `{% include "partials/foo.html" %}`, all variables used in the partial must exist on the parent template struct. Compile error `no field X on type Y` means the parent struct is missing the field, not the partial.

## Askama — JSON in `<script>` blocks

- **Always use `|safe` for JSON**: `{{ json_var|safe }}` in `<script type="application/json">` — Askama's default escaping turns `"` into `&#34;`, breaking `JSON.parse()`. Note: `textContent` does NOT HTML-decode entities; only `innerHTML` would. So the fix is always `|safe` on the server side.

## Actix-web 4

- **Route order matters**: `/users/new` must be registered BEFORE `/users/{id}` or path param swallows "new"
- **Session cookie key**: `Key::generate()` invalidates all sessions on restart — load from env in production
- **`serde_urlencoded` doesn't support duplicate keys**: HTML checkboxes with same `name` fail with `web::Form`. Use custom `parse_form_body()` for repeated fields (see `role_handlers/helpers.rs`)
- **Middleware `from_fn` needs `'static`**: `Next<impl MessageBody + 'static>`, not without lifetime
- **Per-route body size limit**: Use a sub-scope: `web::scope("/api/data").app_data(web::JsonConfig::default().limit(50 * 1024 * 1024))` — setting it on individual `.route()` calls has no effect

## CSS

- **`display:flex` class overrides `[hidden]` attribute**: Browser UA sheet has specificity (0,1,0) for `[hidden]`. A class also has (0,1,0), and source order breaks the tie. If your stylesheet loads after UA, `display:flex` on a class wins. Fix: set `display:none` on the class, add `.class:not([hidden]) { display:flex }` for when it should show.
- **Accordion `scrollHeight` timing**: When using `max-height` transitions for collapse/expand, add the expanded class (which may add padding) BEFORE reading `scrollHeight`. Otherwise the measured height is too small by the padding amount and content clips at the bottom.

## PostgreSQL + sqlx

- **TIMESTAMPTZ → String**: When selecting timestamps into Rust `String` fields, always cast: `e.created_at::TEXT`. Without the cast, sqlx returns a `ColumnDecode` error because `TIMESTAMPTZ` doesn't map to `String`.
- **GROUP BY strictness**: PostgreSQL requires all non-aggregated columns in the GROUP BY clause. Columns from LEFT JOINed tables (e.g., `p_email.value`) must either be in GROUP BY or wrapped in an aggregate like `MAX()`. SQLite was lenient about this.
- **`COALESCE(col, '')` still needed in LEFT JOINs**: sqlx `FromRow` fails on NULL for non-Option `String` types.
- **`STRING_AGG` replaces `GROUP_CONCAT`**: Use `STRING_AGG(DISTINCT col, ',')` for multi-value JOINs. Count queries must use `COUNT(DISTINCT e.id)` to avoid inflated totals.
- **Parameters use `$N` syntax**: `$1`, `$2`, etc. (not `?1`, `?2`).
- **Dynamic SQL table aliases must be consistent**: Search clause `e.name LIKE $1` requires count query to use `FROM entities e` not just `FROM entities`.
- **UNIQUE constraint on `(entity_type, name)`**: PostgreSQL strictly enforces this. Tests that create relation types already in seed data will fail with `23505` duplicate key error. Look up seeded relation types by name instead of re-inserting.
- **All queries are async**: Every `sqlx::query` / model function requires `.await`. Missing `.await` produces "unused Future" warnings at compile time.
- **`query_as` needs explicit column aliases for multi-table SELECTs**: PostgreSQL assigns the bare column name (e.g., `entity_type`) not `table.column`. When two tables share a column name, `FromRow` silently maps the wrong one. Always alias: `SELECT src.entity_type AS source, tgt.entity_type AS target`.
- **Error type misuse in validation**: Don't return `Err(sqlx::Error::RowNotFound)` for input validation failures (e.g., invalid theme value). This error type implies the database row doesn't exist, but the real issue is invalid input. Maps to `AppError::Db` → HTTP 500 instead of 400. Instead: validate before the query and return a handler-appropriate error, or define a validation-specific error type that handlers can remap to HTTP 400.

## EAV Relations

- **`relation::create()` takes relation type name as string, not ID**: Use `relation::create(&pool, "relation_name", source_id, target_id).await` — the function looks up the relation type internally by name. DO NOT try to pass the relation type ID. This is a common mistake when first creating relations programmatically (e.g. in tests or handlers).
