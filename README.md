# 🔗 URL Shortener API

Backend service rút gọn URL, xây dựng bằng **Rust + Actix-web**, lưu trữ dữ liệu với **SQLite**, xác thực người dùng qua **JWT**.

---

## 📋 Tính năng

- Đăng ký / đăng nhập tài khoản với mật khẩu được mã hóa bcrypt
- Tạo short URL với mã ngẫu nhiên 6 ký tự (nanoid)
- Redirect từ short code về URL gốc, ghi nhận lượt click
- Xem thống kê chi tiết: tổng click, click theo ngày, 50 click gần nhất kèm IP và User-Agent
- JWT Bearer token, thời hạn 24 giờ
- Migration database tự động khi khởi động
- Hỗ trợ CORS cấu hình qua biến môi trường

---

## 🏗️ Kiến trúc

```
server/
├── src/
│   ├── main.rs              # Khởi động server, cấu hình routes và middleware
│   ├── models.rs            # Struct Claims (JWT)
│   ├── routes/
│   │   ├── auth.rs          # POST /auth/register, POST /auth/login
│   │   └── url.rs           # POST /api/shorten, GET /{code}, GET /api/stats/{code}
│   ├── db/
│   │   └── analytics.rs     # Query thống kê click theo ngày
│   └── middleware/
│       └── auth.rs          # JWT bearer validator
├── migrations/
│   ├── 001_create_users.sql
│   ├── 002_create_urls.sql
│   ├── 003_create_clicks.sql
│   └── 004_add_indexes.sql
├── Dockerfile
├── Cargo.toml
└── .env
```

### Database schema

```
users       id, username (unique), password_hash, created_at
urls        id, user_id (FK), short_code (unique), original_url, created_at
clicks      id, url_id (FK), clicked_at, ip_address, user_agent
```

---

## ⚙️ Cấu hình môi trường

### Tạo file `.env`

**Windows (PowerShell):**
```powershell
@"
DATABASE_URL=sqlite://data/urls.db
JWT_SECRET=supersecretjwtkey2024
PORT=8080
BASE_URL=http://localhost:8080
ALLOWED_ORIGIN=http://localhost:3000
"@ | Out-File -FilePath "server\.env" -Encoding utf8
```

**Linux / Mac:**
```bash
cat > server/.env << EOF
DATABASE_URL=sqlite://data/urls.db
JWT_SECRET=supersecretjwtkey2024
PORT=8080
BASE_URL=http://localhost:8080
ALLOWED_ORIGIN=http://localhost:3000
EOF
```

File `.env` sau khi tạo ở thư mục `server/`:

```env
DATABASE_URL=sqlite://data/urls.db
JWT_SECRET=supersecretjwtkey2024
PORT=8080
BASE_URL=http://localhost:8080
ALLOWED_ORIGIN=http://localhost:3000
```

| Biến | Mô tả | Mặc định |
|------|-------|----------|
| `DATABASE_URL` | Đường dẫn SQLite | bắt buộc |
| `JWT_SECRET` | Khóa ký JWT | bắt buộc |
| `PORT` | Cổng server | `8080` |
| `BASE_URL` | Base URL để tạo short link | `http://localhost:8080` |
| `ALLOWED_ORIGIN` | Origin được phép CORS | `http://localhost:3000` |

---

## 🚀 Chạy với Docker (khuyến nghị)

```bash
# Build và khởi động
docker compose up --build

# Dừng
docker compose down
```

Server sẽ chạy tại `http://localhost:8080`. Database được lưu tự động vào thư mục `data/`.

## 🛠️ Chạy thủ công (cần cài Rust)

```bash
cd server
cargo run
```

---

## 📡 API Reference

### `GET /health`
Kiểm tra server còn sống không.

**Response:**
```json
{ "status": "ok" }
```

---

### `POST /auth/register`
Đăng ký tài khoản mới.

**Body:**
```json
{
  "username": "testuser",
  "password": "mypassword123"
}
```

**Response `201`:**
```json
{ "message": "User registered successfully" }
```

**Response `409`** — username đã tồn tại:
```json
{ "error": "Username already taken" }
```

---

### `POST /auth/login`
Đăng nhập, nhận JWT token.

**Body:**
```json
{
  "username": "testuser",
  "password": "mypassword123"
}
```

**Response `200`:**
```json
{ "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9..." }
```

**Response `401`** — sai thông tin:
```json
{ "error": "Invalid credentials" }
```

---

### `POST /api/shorten` 🔒
Tạo short URL. **Yêu cầu JWT.**

**Header:**
```
Authorization: Bearer <token>
```

**Body:**
```json
{
  "original_url": "https://www.example.com/very/long/url"
}
```

**Response `200`:**
```json
{
  "short_code": "shBbd0",
  "short_url": "http://localhost:8080/shBbd0"
}
```

**Lưu ý:** `original_url` phải bắt đầu bằng `http://` hoặc `https://`.

---

### `GET /{code}`
Redirect đến URL gốc. Ghi nhận IP và User-Agent của người truy cập.

**Response `301`** — chuyển hướng đến `original_url`.

**Response `404`** — không tìm thấy short code:
```json
{ "error": "Short URL not found" }
```

---

### `GET /api/stats/{code}` 🔒
Xem thống kê lượt click của một short URL. **Yêu cầu JWT, chỉ xem được URL do chính mình tạo.**

**Header:**
```
Authorization: Bearer <token>
```

**Response `200`:**
```json
{
  "short_code": "shBbd0",
  "original_url": "https://www.example.com",
  "created_at": "2024-01-15T10:30:00",
  "total_clicks": 42,
  "clicks_per_day": [
    { "day": "2024-01-15", "count": 20 },
    { "day": "2024-01-16", "count": 22 }
  ],
  "recent_clicks": [
    {
      "clicked_at": "2024-01-16T14:22:00",
      "ip_address": "127.0.0.1",
      "user_agent": "Mozilla/5.0 ..."
    }
  ]
}
```

---

## 🧪 Test bằng PowerShell

```powershell
$base = "http://localhost:8080"

# 1. Đăng ký
Invoke-RestMethod -Uri "$base/auth/register" -Method POST `
    -ContentType "application/json" `
    -Body (@{ username = "testuser"; password = "pass123" } | ConvertTo-Json)

# 2. Đăng nhập - lấy token
$token = (Invoke-RestMethod -Uri "$base/auth/login" -Method POST `
    -ContentType "application/json" `
    -Body (@{ username = "testuser"; password = "pass123" } | ConvertTo-Json)).token

# 3. Tạo short URL
$result = Invoke-RestMethod -Uri "$base/api/shorten" -Method POST `
    -ContentType "application/json" `
    -Headers @{ Authorization = "Bearer $token" } `
    -Body (@{ original_url = "https://example.com" } | ConvertTo-Json)

$code = $result.short_code

# 4. Test redirect
Invoke-WebRequest -Uri "$base/$code" -MaximumRedirection 0

# 5. Xem thống kê
Invoke-RestMethod -Uri "$base/api/stats/$code" -Method GET `
    -Headers @{ Authorization = "Bearer $token" }
```

---

## 🔧 Stack công nghệ

| Thành phần | Công nghệ |
|-----------|-----------|
| Language | Rust 1.88 |
| Web framework | Actix-web 4 |
| Database | SQLite (qua SQLx 0.7) |
| Authentication | JWT (jsonwebtoken 9) |
| Password hashing | bcrypt |
| ID generation | nanoid (6 ký tự) |
| Container | Docker multi-stage build |

---

## ⚠️ Lưu ý

- Chỉ người tạo short URL mới xem được thống kê của URL đó.
- Token JWT có hiệu lực 24 giờ kể từ lúc đăng nhập.
- Cần đảm bảo `BASE_URL` trong `.env` khớp với địa chỉ thực của server, nếu không `short_url` trả về sẽ bị sai.