# URL Shortener – Backend (server/)

Actix-web 4 REST API in Rust. Handles URL shortening, redirects, click tracking, and JWT auth.
Member A owns this package. Members B/C/D work in frontend/, migrations/, and CI respectively.

## Commands

```bash
cargo run -p server                   # start dev server on :8080
cargo test -p server                  # run unit tests
cargo clippy -p server -- -D warnings # lint
cargo build -p server --release       # production build
```

Database (run from workspace root, Member C owns migrations):
```bash
sqlx migrate run                      # apply pending migrations
sqlx migrate revert                   # rollback last migration
```

## Architecture

```
server/src/
  main.rs          # App entrypoint, registers all routes and middleware
  routes/
    url.rs         # POST /shorten · GET /{code} · GET /stats/{code}
    auth.rs        # POST /auth/register · POST /auth/login  (Member C provides JWT logic)
  models/
    url.rs         # Url struct, ShortenRequest, ShortenResponse
    click.rs       # Click struct + log_click() helper
    user.rs        # User struct (shared with Member C)
  middleware/
    auth.rs        # JWT extractor middleware (Member C writes, A integrates)
  db.rs            # SqlitePool initialisation, injected via web::Data
```

Workspace root has `frontend/` (Member B) and `migrations/` (Member C) alongside `server/`.

## Key rules

- All DB access goes through `web::Data<SqlitePool>` — never create a pool inside a handler
- Use `sqlx::query!()` macro (compile-time checked) for every query, never raw string interpolation
- HTTP 301 (not 302) for short-code redirects
- Log every redirect: timestamp (UTC RFC3339), peer IP, User-Agent — insert into `clicks` table
- Protect POST /shorten with JWT middleware; GET /{code} is public
- `nanoid!(6)` generates short codes; regenerate on collision (check DB before insert)
- Return `application/json` for all API errors with `{"error": "..."}` shape

## CORS & static files (Week 5)

- Serve `../frontend/dist/` via `actix-files::Files` at root `/`
- Allow origin `http://localhost:3000` in dev; production origin read from `ALLOWED_ORIGIN` env var
- Required allowed headers: `Authorization`, `Content-Type`

## Environment variables

Read via `dotenvy` at startup. File `.env` is git-ignored — never commit it.

```
DATABASE_URL=sqlite://./dev.db
JWT_SECRET=changeme
ALLOWED_ORIGIN=http://localhost:3000
PORT=8080
```

## Interfaces with other members

- **Member C** provides: `migrations/`, `src/middleware/auth.rs`, JWT validation logic
- **Member B** provides: `frontend/dist/` static build, calls `POST /shorten` and `GET /stats/{code}`
- **Member D** provides: `.github/workflows/ci.yml`; notify D when build commands change