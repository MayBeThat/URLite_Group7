# URLite — URL Shortener with Click Analytics

> Final project for **Nature of Programming Languages** · Group 7
> A fast, self-hosted URL shortener built with Rust + Actix-web.

## ✨ Features

- Shorten long URLs to 6-character codes
- JWT-authenticated user accounts (bcrypt password hashing)
- Dashboard listing all your links with per-link click counts
- Per-link analytics: clicks over time chart, top User-Agents, recent clicks
- Click tracking via HTTP 302 redirect (records IP + User-Agent)
- Single Rust binary serves both API and the static frontend

## 🛠 Tech Stack

- **Backend**: Rust · Actix-web 4 · SQLx (SQLite) · jsonwebtoken · bcrypt · nanoid
- **Frontend**: Vanilla HTML/CSS/JS · Chart.js (CDN) · Open Sans
- **No build step** — frontend is plain files served by the Rust server.

## 🚀 Quick Start

### Prerequisites

- **Rust 1.75+** — install via [rustup](https://rustup.rs/)
- **SQLite** — comes preinstalled on macOS/Linux
- **sqlx-cli** for migrations:
  ```bash
  cargo install sqlx-cli --no-default-features --features native-tls,sqlite
  ```

### 1. Clone the repo

```bash
git clone https://github.com/MayBeThat/URLite_Group7.git
cd URLite_Group7
```

### 2. Create `server/.env`

```env
DATABASE_URL=sqlite:///ABSOLUTE/PATH/TO/server/urls.db
JWT_SECRET=replace-with-a-random-64-char-hex-string
ALLOWED_ORIGIN=http://localhost:3000
PORT=8080
```

Generate a secure JWT secret:

```bash
python3 -c "import secrets; print(secrets.token_hex(32))"
```

### 3. Create DB + run migrations

```bash
cd server
sqlx database create
sqlx migrate run
cd ..
```

### 4. Run the server

```bash
cargo run -p server
```

Open <http://localhost:8080> in your browser.

### 5. Run tests

```bash
cargo test -p server
```

## 📡 API Endpoints

| Method | Path              | Auth | Description                                |
| ------ | ----------------- | :--: | ------------------------------------------ |
| GET    | `/health`         |  –   | Health check                               |
| POST   | `/auth/register`  |  –   | Create a user account                      |
| POST   | `/auth/login`     |  –   | Login, returns JWT                         |
| POST   | `/shorten`        |  ✅  | Create a short URL                         |
| GET    | `/{code}`         |  –   | Redirect to original URL (302) + log click |
| GET    | `/stats/{code}`   |  ✅  | Click stats for a short URL                |
| GET    | `/urls`           |  ✅  | List all your short URLs                   |
| DELETE | `/urls/{code}`    |  ✅  | Delete a short URL (owner only)            |

Authenticated requests must include the header `Authorization: Bearer <JWT>`.

## 📂 Project Structure

```
url-shortener/
├── server/                  # Rust backend
│   ├── src/
│   │   ├── main.rs          # Entry + route registration + CORS
│   │   ├── models.rs        # JWT Claims
│   │   ├── routes/
│   │   │   ├── auth.rs      # /auth/register · /auth/login
│   │   │   └── url.rs       # /shorten · /{code} · /stats · /urls
│   │   ├── db/              # (stub for future analytics queries)
│   │   └── middleware/      # (stub for future JWT middleware)
│   └── migrations/          # SQLx migrations (users, urls, clicks)
├── frontend/dist/           # Static SPA served at /
│   ├── index.html
│   ├── style.css
│   └── app.js
└── Cargo.toml               # Workspace root
```

## 👥 Team — Group 7

| Member            | Role                                         |
| ----------------- | -------------------------------------------- |
| Đặng Hoàng Tân    | Backend (Actix-web, routing, SQLx)           |
| Nguyễn Trần Minh  | Frontend (HTML/CSS/JS, Chart.js)             |
| Nguyễn Đức Long   | Database (schema, migrations, JWT)           |
| Ngô Duy Hoàng     | DevOps (GitHub, CI, deployment)              |

## 📄 License

MIT — Final project for educational purposes.
