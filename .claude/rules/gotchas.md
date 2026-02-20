# Gotchas & Quirks

## Askama 0.14

- **No `ref` in `if let`**: Use `{% if let Some(x) = val %}` not `{% if let Some(ref x) = val %}`
- **Included templates share parent scope**: Every template struct must carry fields used by included partials (e.g. `username`, `permissions` for nav)
- **String equality in loops**: Use `.as_str()` on both sides: `{% if field.as_str() == item.as_str() %}`
- **Can't call `Vec<String>::contains()` with `&str`**: Create wrapper types with template-friendly methods (e.g. `Permissions::has(&str)`)
- **No `&&` in `{% if %}` conditions**: Use nested `{% if a %}{% if b %}...{% endif %}{% endif %}` instead
- **No `||` in `{% if %}` conditions**: Use `{% if a %}...{% else %}{% if b %}...{% endif %}{% endif %}` — duplicate the inner block in both branches
- **No array indexing `arr[i]`**: Can't write `steps[idx-1]`. Find target by ID server-side, determine neighbour, then swap.

## Askama — JSON in `<script>` blocks

- **Always use `|safe` for JSON**: `{{ json_var|safe }}` in `<script type="application/json">` — Askama's default escaping turns `"` into `&#34;`, breaking `JSON.parse()`. Note: `textContent` does NOT HTML-decode entities; only `innerHTML` would. So the fix is always `|safe` on the server side.

## Actix-web 4

- **Route order matters**: `/users/new` must be registered BEFORE `/users/{id}` or path param swallows "new"
- **Session cookie key**: `Key::generate()` invalidates all sessions on restart — load from env in production
- **`serde_urlencoded` doesn't support duplicate keys**: HTML checkboxes with same `name` fail with `web::Form`. Use custom `parse_form_body()` for repeated fields (see `role_handlers/helpers.rs`)
- **Middleware `from_fn` needs `'static`**: `Next<impl MessageBody + 'static>`, not without lifetime
- **Blocking ops on async thread**: SQLite calls in async handlers stall Actix workers. Wrap heavy sync work with `web::block(move || heavy_fn(&conn, &data)).await.map_err(|e| AppError::Session(...))?`
- **Per-route body size limit**: Use a sub-scope: `web::scope("/api/data").app_data(web::JsonConfig::default().limit(50 * 1024 * 1024))` — setting it on individual `.route()` calls has no effect

## CSS

- **`display:flex` class overrides `[hidden]` attribute**: Browser UA sheet has specificity (0,1,0) for `[hidden]`. A class also has (0,1,0), and source order breaks the tie. If your stylesheet loads after UA, `display:flex` on a class wins. Fix: set `display:none` on the class, add `.class:not([hidden]) { display:flex }` for when it should show.

## SQLite + r2d2

- **Create parent dirs first**: `fs::create_dir_all("data")` before pool init
- **WAL pragma is per-connection**: Set via `SqliteConnectionManager::file(path).with_init(...)`
- **`COALESCE(col, '')` required in LEFT JOINs**: rusqlite `row.get()` fails on NULL for non-Option types
- **Dynamic SQL table aliases must be consistent**: Search clause `e.name LIKE ?1` requires count query to use `FROM entities e` not just `FROM entities`
- **GROUP_CONCAT for multi-value JOINs**: When a LEFT JOIN produces N rows per entity (e.g., user with N roles), use `GROUP_CONCAT(DISTINCT col)` + `GROUP BY e.id`. Count queries must use `COUNT(DISTINCT e.id)` to avoid inflated totals.

## EAV Relations

- **`relation::create()` takes relation type name as string, not ID**: Use `relation::create(&conn, "relation_name", source_id, target_id)` — the function looks up the relation type internally by name. DO NOT try to pass the relation type ID. This is a common mistake when first creating relations programmatically (e.g. in tests or handlers).
