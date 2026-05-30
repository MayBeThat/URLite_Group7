# URLite — Project Summary

**URL Shortener with Analytics Dashboard**
**Group 7 | CS-370 | 4 members | 6 weeks**

---

## Thành viên & Phân công

| Tên | Vai trò |
|-----|---------|
| Đặng Hoàng Tân | Backend — server, routing, SQLx, shorten/redirect, CORS |
| Nguyễn Trần Minh | Frontend — HTML/JS/CSS, UI, Chart.js |
| Nguyễn Đức Long | Database — schema, migrations, JWT middleware, analytics |
| Ngô Duy Hoàng | DevOps — GitHub, CI, testing, deployment, README |

---

## Tech Stack

| Layer | Công nghệ |
|-------|-----------|
| Language | Rust (edition 2024) |
| Web framework | Actix-web 4 |
| Database | SQLite (via SQLx 0.7) |
| Auth | JWT (jsonwebtoken 9, HS256) + bcrypt 0.15 |
| Middleware | actix-web-httpauth 0.8 |
| Frontend | HTML / CSS / JavaScript + Chart.js |
| Static files | actix-files 0.6 |
| CORS | actix-cors 0.7 |
| Async runtime | Tokio 1 (full features) |
| ID generation | nanoid 0.4 (6-char short codes) |
| Serialization | serde + serde_json |
| Time | chrono 0.4 |
| Config | dotenvy 0.15 (.env file) |

---

## Cấu trúc project

```
URLite_Group7/
├── Cargo.toml                  # Workspace root
├── server/
│   ├── Cargo.toml              # Server dependencies
│   ├── .env                    # DATABASE_URL, JWT_SECRET, PORT
│   ├── urls.db                 # SQLite database file
│   ├── migrations/
│   │   ├── 001_create_users.sql
│   │   ├── 002_create_urls.sql
│   │   ├── 003_create_clicks.sql
│   │   └── 004_add_indexes.sql
│   └── src/
│       ├── main.rs             # App entry point, route wiring
│       ├── models.rs           # Shared types (Claims)
│       ├── middleware/
│       │   ├── mod.rs
│       │   └── auth.rs         # JWT bearer validator
│       ├── db/
│       │   ├── mod.rs
│       │   └── analytics.rs    # get_daily_clicks, get_url_stats
│       └── routes/
│           ├── mod.rs
│           ├── auth.rs         # POST /auth/register, POST /auth/login
│           └── url.rs          # POST /api/shorten, GET /{code}, GET /api/stats/{code}
└── URLite/                     # Frontend (chưa implement)
```

---

## Database Schema

### `users`
| Column | Type | Ghi chú |
|--------|------|---------|
| id | INTEGER PK AUTOINCREMENT | |
| username | TEXT UNIQUE NOT NULL | |
| password_hash | TEXT NOT NULL | bcrypt hash |
| created_at | TEXT DEFAULT datetime('now') | |

### `urls`
| Column | Type | Ghi chú |
|--------|------|---------|
| id | INTEGER PK AUTOINCREMENT | |
| user_id | INTEGER FK → users(id) | ON DELETE CASCADE |
| short_code | TEXT UNIQUE NOT NULL | 6-char nanoid |
| original_url | TEXT NOT NULL | |
| created_at | TEXT DEFAULT datetime('now') | |

### `clicks`
| Column | Type | Ghi chú |
|--------|------|---------|
| id | INTEGER PK AUTOINCREMENT | |
| url_id | INTEGER FK → urls(id) | ON DELETE CASCADE |
| clicked_at | TEXT DEFAULT datetime('now') | |
| ip_address | TEXT | nullable |
| user_agent | TEXT | nullable |

### Indexes (migration 004)
- `idx_urls_short_code` — tìm URL theo code (hot path)
- `idx_urls_user_id` — lọc URL theo user
- `idx_clicks_url_id` — join clicks → urls
- `idx_clicks_clicked_at` — sort/filter theo thời gian
- `idx_clicks_url_date` — composite, tối ưu analytics query

---

## API Endpoints

| Method | Path | Auth | Mô tả |
|--------|------|------|-------|
| GET | `/health` | ❌ | Health check |
| POST | `/auth/register` | ❌ | Đăng ký tài khoản |
| POST | `/auth/login` | ❌ | Đăng nhập, trả về JWT |
| GET | `/{code}` | ❌ | Redirect đến URL gốc (HTTP 301), log click |
| POST | `/api/shorten` | ✅ JWT | Tạo short URL |
| GET | `/api/stats/{code}` | ✅ JWT | Xem thống kê URL |

### Response mẫu — `GET /api/stats/{code}`
```json
{
  "short_code": "abc123",
  "original_url": "https://example.com",
  "created_at": "2024-01-01 00:00:00",
  "total_clicks": 42,
  "clicks_per_day": [
    { "day": "2024-01-01", "count": 10 },
    { "day": "2024-01-02", "count": 32 }
  ],
  "recent_clicks": [
    { "clicked_at": "...", "ip_address": "1.2.3.4", "user_agent": "Mozilla/..." }
  ]
}
```

---

## Security

- **Password hashing:** bcrypt (DEFAULT_COST) — không lưu plaintext
- **Authentication:** JWT HS256, expire 24h, gửi qua `Authorization: Bearer <token>`
- **Route protection:** `actix-web-httpauth` middleware bọc toàn bộ `/api` scope
- **Claims injection:** Sau khi validate, `Claims` được insert vào request extensions để handler dùng lại, không parse lại JWT
- **Input validation:** `original_url` phải bắt đầu bằng `http://` hoặc `https://`
- **Collision avoidance:** Short code generation retry loop cho đến khi tìm được code chưa tồn tại

---

## Luồng hoạt động chính

```
User → POST /auth/login → nhận JWT token
     → POST /api/shorten (Bearer token) → nhận short_code
     → chia sẻ link: http://localhost:8080/{code}

Visitor → GET /{code} → server redirect 301 → original URL
                      → server log click (ip, user_agent, timestamp)

User → GET /api/stats/{code} (Bearer token)
     → nhận total_clicks + clicks_per_day (dùng cho Chart.js)
```

---

## Kế hoạch 6 tuần

| Tuần | Nội dung | Người phụ trách |
|------|----------|-----------------|
| 1 | Học Rust cơ bản, setup môi trường | Cả nhóm |
| 2 | Actix-web skeleton, DB schema, GitHub CI | Tân / Long / Hoàng |
| 3 | SQLx, auth (register/login), JWT middleware | Tân / Long / Minh |
| 4 | Shorten, redirect, click tracking, analytics queries | Tân / Long / Minh |
| 5 | Chart.js, static files, CORS, DB indexes, deploy | Tất cả |
| 6 | Báo cáo, slides, demo | Tất cả |

---

## Trạng thái hiện tại

| Phần | Trạng thái |
|------|-----------|
| DB migrations (001–004) | ✅ Hoàn thành |
| Auth routes (register/login) | ✅ Hoàn thành |
| JWT middleware | ✅ Hoàn thành |
| URL shorten + redirect | ✅ Hoàn thành |
| Click tracking | ✅ Hoàn thành |
| Analytics queries | ✅ Hoàn thành |
| Stats endpoint với daily chart data | ✅ Hoàn thành |
| DB indexes | ✅ Hoàn thành |
| Unit tests (routes/url.rs) | ✅ Có sẵn |
| Frontend (HTML/Chart.js) | ⏳ Chưa implement |
| Deployment | ⏳ Chưa thực hiện |

---

## Cách chạy project

### Yêu cầu
- Rust stable (≥ 1.75) + Visual C++ Build Tools (Windows)
- File `.env` trong `server/`:

```env
DATABASE_URL=sqlite:///path/to/urls.db
JWT_SECRET=your_secret_key
PORT=8080
ALLOWED_ORIGIN=http://localhost:3000
```

### Chạy server
```bash
cd server
cargo run
```
Migrations tự chạy khi server khởi động. Server lắng nghe tại `http://localhost:8080`.

### Chạy tests
```bash
cd server
cargo test
```
