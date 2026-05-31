# URLite

URL shortener with click analytics. Rust + Actix-web backend, vanilla HTML/CSS/JS frontend, SQLite storage, JWT authentication. A single Docker container serves both the API and the static frontend on port 8080.

## Table of contents

1. Overview
2. Project structure
3. Quick start with Docker
4. Running the server without Docker
5. Running the frontend
6. Environment variables
7. Database
8. API reference
9. Authentication
10. Smoke test with cURL
11. Tech stack
12. Notes and limitations

---

## 1. Overview

URLite turns long URLs into 6-character short links and tracks every click. Each user has their own account; only the owner of a link can view its analytics or delete it. The product is small enough to deploy in one container yet exercises a realistic backend stack — async Rust, JWT, parameterised SQL, structured errors, CORS, and static-file fallthrough.

The backend is intentionally minimal: a single binary that opens SQLite, runs migrations, mounts a few routes, and serves the SPA. The frontend is plain HTML, CSS, and JavaScript — there is no build step, no bundler, and no node dependencies. Everything is served from the Rust process on one port.

The codebase is split across the team: backend in `server/`, frontend in `frontend/dist/`, database migrations under `server/migrations/`, and Docker configuration at the repository root.

---

## 2. Project structure

```
url-shortener/
├── docker-compose.yml          Service definition (build, env vars, volumes, ports)
├── README.md                   This document
├── PROJECT_SUMMARY.md          High-level project summary (separate document)
├── Group7-works.md             Team work breakdown (separate document)
├── URLite/                     Git submodule pointing at Michellein/URLite (a separate
│                               prototype repo). Not used at runtime; preserved so the
│                               pin in .gitmodules remains valid.
├── server/                     Rust backend
│   ├── Cargo.toml              Package manifest and dependency list
│   ├── Dockerfile              Multi-stage build (rust:1.88-slim -> debian:bookworm-slim)
│   ├── .env                    Local environment file, git-ignored
│   ├── data/
│   │   └── urls.db             SQLite database file, created on first start
│   ├── migrations/
│   │   ├── 001_create_users.sql    `users` table (id, username unique, password_hash, created_at)
│   │   ├── 002_create_urls.sql     `urls` table (id, user_id FK, short_code unique, original_url, created_at)
│   │   ├── 003_create_clicks.sql   `clicks` table (id, url_id FK ON DELETE CASCADE, clicked_at, ip_address, user_agent)
│   │   └── 004_add_indexes.sql     Indexes on short_code, user_id, url_id, clicked_at
│   └── src/
│       ├── main.rs             Process bootstrap: read env, open SQLite pool, run
│       │                       migrations, build the CORS layer, register routes, wrap
│       │                       JWT middleware on /shorten and /urls, serve the SPA.
│       ├── error.rs            AppError enum with a ResponseError impl and From impls
│       │                       for sqlx::Error, std::io::Error, and bcrypt::BcryptError,
│       │                       so handlers can use the `?` operator.
│       ├── models.rs           Claims (JWT payload) plus newtype wrappers BaseUrl,
│       │                       JwtSecret, and FrontendDir so multiple String values can
│       │                       coexist in actix-web's app_data without collisions.
│       ├── db/
│       │   ├── mod.rs          Module declarations for analytics and clicks.
│       │   ├── analytics.rs    get_url_stats() — JOINs urls, clicks, and users to
│       │   │                   return short_code, original_url, created_at, total_clicks,
│       │   │                   and the internal url id for a code owned by a username.
│       │   └── clicks.rs       log_click() — pulls peer IP and User-Agent from the
│       │                       request and INSERTs a row into the `clicks` table.
│       ├── middleware/
│       │   ├── mod.rs          Module declaration for auth.
│       │   └── auth.rs         JWT bearer validator. Decodes the token from the
│       │                       Authorization header, validates the signature, and
│       │                       inserts the resulting Claims into the request extensions
│       │                       so downstream handlers can read it without re-decoding.
│       └── routes/
│           ├── mod.rs          Module declarations for auth and url.
│           ├── auth.rs         register and login handlers. register bcrypt-hashes the
│           │                   password and INSERTs into users; login verifies the
│           │                   password and encodes a 24-hour HS256 JWT.
│           └── url.rs          The link-handling endpoints: shorten (generate a unique
│                               nanoid, insert into urls), redirect (lookup by short_code,
│                               log the click, return 301), get_stats (per-link analytics
│                               with the 50 most recent clicks), list_urls (caller's
│                               links with click counts), delete_url (cascades to clicks
│                               via the FK).
└── frontend/
    └── dist/                   Static SPA. No build step; files are served as-is.
        ├── index.html          Page shell with all sections: hero shorten card,
        │                       result banner, dashboard table, stats page, auth modal,
        │                       and the info sections at the bottom.
        ├── style.css           All CSS in a single file. Theme variables, hero,
        │                       shorten card, buttons, dashboard table, stats charts,
        │                       footer, and responsive breakpoints.
        └── app.js              All client logic. Page navigation, the apiRequest()
                                fetch wrapper that auto-adds the Authorization header,
                                login/register/logout flows, shorten/delete/list/stats
                                handlers, and the Chart.js render call for analytics.
```

---

## 3. Quick start with Docker

Prerequisites:

- Docker Desktop 4.x (macOS, Windows) or Docker Engine 20+ (Linux).
- On Windows, Docker Desktop requires the WSL2 backend, which is enabled by default in recent installers. If you do not have WSL2, follow Microsoft's installation guide before running Docker Desktop.
- Compose v2 is bundled with Docker Desktop and Docker Engine 20.10+; no separate install is needed.

From the repository root:

```
docker compose up --build -d
```

The container is named `urlite-server`. Open `http://localhost:8080` in a browser. The compose file mounts two host paths into the container so changes persist across rebuilds:

- `./server/data` mounted at `/app/data` — the SQLite database file
- `./frontend/dist` mounted at `/app/frontend` — UI assets, read-only

Common operations:

```
docker compose logs -f server          Tail server logs
docker compose restart server          Restart without rebuilding the image
docker compose down                    Stop and remove the container
docker compose build --no-cache        Force a clean rebuild
docker exec -it urlite-server /bin/sh  Open a shell inside the container
```

---

## 4. Running the server without Docker

Prerequisites:

- Rust 1.88 or newer:
  - macOS / Linux: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
  - Windows: download and run [`rustup-init.exe`](https://www.rust-lang.org/tools/install). Accept the default host triple `x86_64-pc-windows-msvc` when prompted. The installer will offer to fetch the Visual Studio Build Tools if they are missing; accept this.
- A working C linker:
  - macOS: Xcode Command Line Tools (`xcode-select --install`).
  - Linux: `gcc` or `clang` from your distribution.
  - Windows: included with the Visual Studio Build Tools installed above.

Open a terminal at the repository root and switch into the server crate:

```
cd server
```

Create `server/.env`. Pick the snippet that matches your shell.

bash / zsh / Git Bash (macOS, Linux, or Windows with Git for Windows):

```
cat > .env <<'EOF'
DATABASE_URL=sqlite://data/urls.db?mode=rwc
JWT_SECRET=replace-me-with-a-real-secret
PORT=8080
BASE_URL=http://localhost:8080
ALLOWED_ORIGIN=http://localhost:8080
FRONTEND_DIR=../frontend/dist
EOF
```

PowerShell (Windows):

```
@'
DATABASE_URL=sqlite://data/urls.db?mode=rwc
JWT_SECRET=replace-me-with-a-real-secret
PORT=8080
BASE_URL=http://localhost:8080
ALLOWED_ORIGIN=http://localhost:8080
FRONTEND_DIR=../frontend/dist
'@ | Out-File -FilePath .env -Encoding utf8
```

Then build and run:

```
cargo run                       Debug build, prints "Server running at ..." when ready
cargo build --release           Optimized binary at target/release/server
cargo clippy -- -D warnings     Run the linter and fail on warnings
```

The server reads `server/.env` via `dotenvy`, opens the SQLite file in `DATABASE_URL`, applies migrations, and listens on `PORT`. `FRONTEND_DIR` must point at the static assets if you also want the server to serve the SPA at `GET /`. Without it the server defaults to the Docker path `/app/frontend`, which does not exist on the host, and `GET /` will return 404.

---

## 5. Running the frontend

The frontend is plain HTML, CSS, and JavaScript with no build step, no `node_modules`, and no bundler. There are two ways to view it.

(a) Through the backend (recommended)

When the Rust server is running, either via Docker or `cargo run` with `FRONTEND_DIR` set, the server returns `index.html` for `GET /` and any unknown single-segment path falls through to a static file in `FRONTEND_DIR`. Open `http://localhost:8080` and the SPA loads.

(b) Directly with a static HTTP server

You can serve `frontend/dist/` with any static server, for example:

- macOS / Linux: `python3 -m http.server` from inside `frontend/dist/`.
- Windows (PowerShell): `python -m http.server` from inside `frontend/dist/`. The default Python on Windows is `python`, not `python3`.
- VS Code: install the Live Server extension and right-click `index.html` -> "Open with Live Server".

This loads the SPA from a different origin than the backend (port 8000 or 5500 vs 8080), so the relative paths in `app.js` (`/shorten`, `/urls`, ...) will hit the static server, not the API, and every request fails. Use mode (a) for any meaningful testing.

Editing the frontend is just editing the files in `frontend/dist/` and reloading the browser. When running under Docker the bind mount makes changes visible immediately; you do not need to rebuild the image.

---

## 6. Environment variables

Set inline in `docker-compose.yml` for the Docker workflow, or in `server/.env` for `cargo run`.

| Variable        | Required | Default                  | Description |
|-----------------|----------|--------------------------|-------------|
| DATABASE_URL    | Yes      | -                        | SQLite URI, for example `sqlite:///app/data/urls.db?mode=rwc`. The `?mode=rwc` parameter lets SQLx create the file if it does not exist. |
| JWT_SECRET      | Yes      | -                        | HMAC secret used to sign JWT tokens. Replace before any deployment. |
| PORT            | No       | 8080                     | HTTP listen port. |
| BASE_URL        | No       | http://localhost:{PORT}  | Origin used to construct the `short_url` field in API responses. |
| ALLOWED_ORIGIN  | No       | http://localhost:8080    | Value sent in the CORS `Access-Control-Allow-Origin` header. Matches `BASE_URL` by default because the backend serves both the API and the SPA from the same origin. |
| FRONTEND_DIR    | No       | /app/frontend            | Filesystem path served for `GET /` and the static-file fallback in the redirect handler. |

---

## 7. Database

SQLite single-file database stored at `server/data/urls.db`. Schema and indexes are defined by the SQL files under `server/migrations/` and applied on startup via `sqlx::migrate!`.

| Table  | Columns |
|--------|---------|
| users  | id, username (unique), password_hash, created_at |
| urls   | id, user_id (FK to users), short_code (unique), original_url, created_at |
| clicks | id, url_id (FK to urls ON DELETE CASCADE), clicked_at, ip_address, user_agent |

Inspect the database with the `sqlite3` CLI:

```
sqlite3 server/data/urls.db ".tables"
sqlite3 server/data/urls.db "SELECT id, username FROM users;"
sqlite3 server/data/urls.db "SELECT short_code, original_url FROM urls ORDER BY id DESC LIMIT 5;"
```

macOS and most Linux distributions ship with `sqlite3` preinstalled. On Windows, download the precompiled `sqlite-tools-win-x64-*.zip` from [sqlite.org/download.html](https://www.sqlite.org/download.html) and place `sqlite3.exe` on your `PATH`. As an alternative that works on any host, run the queries through the running container:

```
docker exec urlite-server sqlite3 /app/data/urls.db ".tables"
```

---

## 8. API reference

All requests and responses use JSON. Endpoints marked `(auth)` require an `Authorization: Bearer <token>` header obtained from `POST /auth/login`.

| Method | Path           | Auth | Description |
|--------|----------------|------|-------------|
| GET    | /              | No   | Serves the SPA (`index.html`). |
| GET    | /health        | No   | Liveness check. Returns `{"status":"ok"}`. |
| POST   | /auth/register | No   | Create a new user. |
| POST   | /auth/login    | No   | Exchange credentials for a JWT. |
| POST   | /shorten       | Yes  | Create a new short URL for the caller. |
| GET    | /{code}        | No   | 301 redirect to the original URL; logs a click. Falls through to a static file in `FRONTEND_DIR` if `{code}` is not a known short code. |
| GET    | /stats/{code}  | Yes  | Analytics for a URL owned by the caller. |
| GET    | /urls          | Yes  | List the caller's URLs with click counts. |
| DELETE | /urls/{code}   | Yes  | Delete a URL owned by the caller; clicks cascade. |

### POST /auth/register

Request:
```
{ "username": "alice", "password": "secret123" }
```
Responses:
- `201 { "message": "User registered successfully" }`
- `409 { "error": "Username already taken" }`

### POST /auth/login

Request:
```
{ "username": "alice", "password": "secret123" }
```
Responses:
- `200 { "token": "<jwt>" }`
- `401 { "error": "Invalid credentials" }`

### POST /shorten (auth)

Request:
```
{ "original_url": "https://example.com/some/long/path" }
```
The URL must start with `http://` or `https://`.

Response:
```
{
  "short_code": "abc123",
  "short_url": "http://localhost:8080/abc123"
}
```

### GET /{code}

- `301` with `Location: <original_url>` on success; the click is recorded.
- If `{code}` matches a file in `FRONTEND_DIR`, the file is returned with a 200.
- `404 { "error": "Short URL not found" }` otherwise.

### GET /stats/{code} (auth)

Response:
```
{
  "short_code": "abc123",
  "original_url": "https://example.com",
  "created_at": "2026-05-31T06:24:56",
  "click_count": 12,
  "clicks": [
    { "clicked_at": "...", "ip_address": "...", "user_agent": "..." }
  ]
}
```
`clicks` contains up to the 50 most recent clicks. The frontend computes the daily-breakdown chart from this array client-side.

### GET /urls (auth)

Response is an array sorted by `created_at` descending:
```
[
  {
    "short_code": "abc123",
    "short_url": "http://localhost:8080/abc123",
    "original_url": "https://example.com",
    "created_at": "2026-05-31 06:24:56",
    "click_count": 3
  }
]
```

### DELETE /urls/{code} (auth)

- `204 No Content` on success.
- `404 { "error": "Short URL not found" }` if the URL does not exist or is not owned by the caller.

---

## 9. Authentication

Authentication is JWT bearer with the following properties:

- Algorithm: HS256.
- Secret: the `JWT_SECRET` environment variable.
- Expiry: 24 hours from `iat`. There is no refresh token.
- Claims: `{ "sub": "<username>", "exp": <unix-timestamp> }`.

The token is issued by `POST /auth/login` and must be sent on protected requests as `Authorization: Bearer <token>`. Validation is performed by the middleware in `server/src/middleware/auth.rs`, which inserts the decoded `Claims` into the request extensions. The validator is wrapped around two scopes in `main.rs`: `/shorten` and `/urls`. The `GET /stats/{code}` handler reads the token manually so that it can also be called from contexts that did not pass through the wrapped scope.

Passwords are hashed with `bcrypt` at the default cost (12) before storage. The plaintext password is never stored or logged.

---

## 10. Smoke test with cURL

This sequence exercises every endpoint end-to-end. `curl` is available on macOS, Linux, and Windows 10+ (Windows ships `curl.exe`, which is the real curl). Pick the block that matches your shell.

bash / zsh (macOS, Linux, Git Bash on Windows). Requires `jq` for parsing the JWT (install via `brew install jq`, `apt install jq`, or [jqlang.github.io/jq](https://jqlang.github.io/jq/)):

```
BASE=http://localhost:8080

# Register a new user
curl -X POST $BASE/auth/register \
  -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"secret123"}'

# Log in and store the token
TOKEN=$(curl -s -X POST $BASE/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"secret123"}' | jq -r .token)

# Create a short URL
curl -X POST $BASE/shorten \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"original_url":"https://example.com"}'

# Visit the short URL (logs a click and returns 301)
curl -I $BASE/<short_code>

# Inspect analytics
curl -H "Authorization: Bearer $TOKEN" $BASE/stats/<short_code>

# List the caller's URLs
curl -H "Authorization: Bearer $TOKEN" $BASE/urls

# Delete the URL
curl -X DELETE -H "Authorization: Bearer $TOKEN" $BASE/urls/<short_code>
```

PowerShell (Windows). Uses `curl.exe` to avoid the built-in `Invoke-WebRequest` alias and `ConvertFrom-Json` instead of `jq`:

```
$BASE = "http://localhost:8080"

# Register
curl.exe -X POST "$BASE/auth/register" `
  -H "Content-Type: application/json" `
  -d '{"username":"alice","password":"secret123"}'

# Log in and store the token
$TOKEN = (curl.exe -s -X POST "$BASE/auth/login" `
  -H "Content-Type: application/json" `
  -d '{"username":"alice","password":"secret123"}' | ConvertFrom-Json).token

# Create a short URL
curl.exe -X POST "$BASE/shorten" `
  -H "Authorization: Bearer $TOKEN" `
  -H "Content-Type: application/json" `
  -d '{"original_url":"https://example.com"}'

# Visit the short URL (logs a click and returns 301)
curl.exe -I "$BASE/<short_code>"

# Inspect analytics
curl.exe -H "Authorization: Bearer $TOKEN" "$BASE/stats/<short_code>"

# List the caller's URLs
curl.exe -H "Authorization: Bearer $TOKEN" "$BASE/urls"

# Delete the URL
curl.exe -X DELETE -H "Authorization: Bearer $TOKEN" "$BASE/urls/<short_code>"
```

---

## 11. Tech stack

| Layer            | Technology |
|------------------|------------|
| Language         | Rust 1.88 |
| Web framework    | Actix-web 4 |
| Static files     | actix-files 0.6 |
| Database driver  | SQLx 0.7 (sqlite, runtime-tokio-native-tls, chrono, macros) |
| Auth             | jsonwebtoken 9 (HS256, 24 hour expiry), bcrypt 0.15 |
| Short codes      | nanoid 0.4 (6 characters from a URL-safe alphabet) |
| Frontend         | Vanilla JavaScript, HTML, CSS. Chart.js loaded from CDN. |
| Container        | Multi-stage Docker (`rust:1.88-slim` builder, `debian:bookworm-slim` runtime) |

---

## 12. Notes and limitations

- The `JWT_SECRET` value in `docker-compose.yml` is a development value. Replace it before any deployment.
- There are no unit or integration tests yet.
- There is no rate limiting on `/auth/login` or `/shorten`.
- Password complexity is not enforced.
- HTTPS is not configured at the server. Place this behind a reverse proxy (nginx, Caddy, Traefik) for TLS termination.
- The `URLite/` submodule is unused at runtime. It points at a separate prototype repo and is kept only because the `.gitmodules` entry was created earlier in the project.
- The frontend uses relative paths for every API call, so it must always be served from the same origin as the backend.
- The project has been used on macOS and Windows. The simplest cross-platform path is `docker compose` (with the WSL2 backend on Windows). The `cargo run` path additionally requires a host Rust toolchain: rustup on macOS / Linux, or `rustup-init.exe` plus the MSVC Build Tools on Windows.
