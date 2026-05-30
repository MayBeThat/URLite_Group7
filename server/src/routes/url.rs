use actix_web::{delete, get, http::StatusCode, post, web, HttpRequest, HttpResponse, Responder};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::models::Claims;

const BASE_URL: &str = "http://localhost:8080";
const INTERNAL_ERR: &str = "Internal server error";

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ShortenRequest {
    pub original_url: String,
}

#[derive(Serialize)]
pub struct ShortenResponse {
    pub short_code: String,
    pub short_url: String,
}

#[derive(Serialize)]
pub struct ClickRecord {
    pub clicked_at: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub short_code: String,
    pub original_url: String,
    pub created_at: String,
    pub click_count: i64,
    pub clicks: Vec<ClickRecord>,
}

#[derive(Serialize)]
pub struct UrlItem {
    pub short_code: String,
    pub short_url: String,
    pub original_url: String,
    pub created_at: String,
    pub click_count: i64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn json_err(status: StatusCode, msg: &str) -> actix_web::Error {
    actix_web::error::InternalError::from_response(
        msg.to_string(),
        HttpResponse::build(status)
            .content_type("application/json")
            .body(serde_json::json!({"error": msg}).to_string()),
    )
    .into()
}

async fn resolve_user_id(db: &SqlitePool, username: &str) -> Result<Option<i64>, actix_web::Error> {
    let row = sqlx::query!("SELECT id FROM users WHERE username = ?", username)
        .fetch_optional(db)
        .await
        .map_err(|_| json_err(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR))?;
    Ok(row.and_then(|r| r.id))
}

fn require_jwt(req: &HttpRequest, jwt_secret: &str) -> Result<Claims, actix_web::Error> {
    let header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| json_err(StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| json_err(StatusCode::UNAUTHORIZED, "Invalid Authorization header format"))?;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| json_err(StatusCode::UNAUTHORIZED, "Invalid or expired token"))?;

    Ok(token_data.claims)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /shorten — create a short URL (JWT required)
#[post("/shorten")]
pub async fn shorten(
    req: HttpRequest,
    body: web::Json<ShortenRequest>,
    db: web::Data<SqlitePool>,
    jwt_secret: web::Data<String>,
) -> Result<impl Responder, actix_web::Error> {
    let claims = require_jwt(&req, &jwt_secret)?;

    if !body.original_url.starts_with("http://") && !body.original_url.starts_with("https://") {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "original_url must start with http:// or https://",
        ));
    }

    let user_id = resolve_user_id(db.get_ref(), &claims.sub).await?;

    // Retry until a collision-free short code is found
    let code = loop {
        let candidate = nanoid::nanoid!(6);
        let exists = sqlx::query!(
            "SELECT id FROM urls WHERE short_code = ?",
            candidate
        )
        .fetch_optional(db.get_ref())
        .await
        .map_err(|_| json_err(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR))?;
        if exists.is_none() {
            break candidate;
        }
    };

    sqlx::query!(
        "INSERT INTO urls (short_code, original_url, user_id) VALUES (?, ?, ?)",
        code,
        body.original_url,
        user_id
    )
    .execute(db.get_ref())
    .await
    .map_err(|_| json_err(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR))?;

    Ok(HttpResponse::Ok().json(ShortenResponse {
        short_url: format!("{}/{}", BASE_URL, code),
        short_code: code,
    }))
}

/// GET /{code} — redirect to original URL and record the click
/// Regex constrains to exactly 6 URL-safe chars so static files are not intercepted
#[get("/{code:[a-zA-Z0-9_\\-]{6}}")]
pub async fn redirect(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<SqlitePool>,
) -> Result<impl Responder, actix_web::Error> {
    let code = path.into_inner();

    let row = sqlx::query!("SELECT id, original_url FROM urls WHERE short_code = ?", code)
        .fetch_optional(db.get_ref())
        .await
        .map_err(|_| json_err(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR))?
        .ok_or_else(|| json_err(StatusCode::NOT_FOUND, "Short URL not found"))?;

    // Record click
    let ip = req
        .connection_info()
        .peer_addr()
        .map(str::to_string);
    let ua = req
        .headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let clicked_at = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        "INSERT INTO clicks (url_id, clicked_at, ip_address, user_agent) VALUES (?, ?, ?, ?)",
        row.id,
        clicked_at,
        ip,
        ua
    )
    .execute(db.get_ref())
    .await
    .map_err(|_| json_err(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR))?;

    Ok(HttpResponse::Found()
        .insert_header(("Location", row.original_url))
        .finish())
}

/// GET /stats/{code} — return stats for a short URL (JWT required)
#[get("/stats/{code}")]
pub async fn get_stats(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<SqlitePool>,
    jwt_secret: web::Data<String>,
) -> Result<impl Responder, actix_web::Error> {
    // Enforce authentication
    let _claims = require_jwt(&req, &jwt_secret)?;

    let code = path.into_inner();

    // Fetch the URL row
    let url_row = sqlx::query!(
        "SELECT id, original_url, created_at FROM urls WHERE short_code = ?",
        code
    )
    .fetch_optional(db.get_ref())
    .await
    .map_err(|_| json_err(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR))?
    .ok_or_else(|| json_err(StatusCode::NOT_FOUND, "Short URL not found"))?;

    // Total click count
    let count_row = sqlx::query!(
        "SELECT COUNT(*) AS cnt FROM clicks WHERE url_id = ?",
        url_row.id
    )
    .fetch_one(db.get_ref())
    .await
    .map_err(|_| json_err(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR))?;

    // Individual click records
    let click_rows = sqlx::query!(
        "SELECT clicked_at, ip_address, user_agent FROM clicks WHERE url_id = ? ORDER BY clicked_at DESC",
        url_row.id
    )
    .fetch_all(db.get_ref())
    .await
    .map_err(|_| json_err(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR))?;

    let clicks: Vec<ClickRecord> = click_rows
        .into_iter()
        .map(|r| ClickRecord {
            clicked_at: r.clicked_at,
            ip_address: r.ip_address,
            user_agent: r.user_agent,
        })
        .collect();

    Ok(HttpResponse::Ok().json(StatsResponse {
        short_code: code,
        original_url: url_row.original_url,
        created_at: url_row.created_at,
        click_count: count_row.cnt as i64,
        clicks,
    }))
}

/// GET /urls — list all URLs created by the authenticated user
#[get("/urls")]
pub async fn list_urls(
    req: HttpRequest,
    db: web::Data<SqlitePool>,
    jwt_secret: web::Data<String>,
) -> Result<impl Responder, actix_web::Error> {
    let claims = require_jwt(&req, &jwt_secret)?;

    let user_id = match resolve_user_id(db.get_ref(), &claims.sub).await? {
        Some(id) => id,
        None => return Ok(HttpResponse::Ok().json(Vec::<UrlItem>::new())),
    };

    let rows = sqlx::query!(
        "SELECT u.short_code, u.original_url, u.created_at, COUNT(c.id) as click_count
         FROM urls u
         LEFT JOIN clicks c ON c.url_id = u.id
         WHERE u.user_id = ?
         GROUP BY u.id
         ORDER BY u.created_at DESC",
        user_id
    )
    .fetch_all(db.get_ref())
    .await
    .map_err(|_| json_err(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR))?;

    let items: Vec<UrlItem> = rows
        .into_iter()
        .map(|r| UrlItem {
            short_url: format!("{}/{}", BASE_URL, r.short_code),
            short_code: r.short_code,
            original_url: r.original_url,
            created_at: r.created_at,
            click_count: r.click_count,
        })
        .collect();

    Ok(HttpResponse::Ok().json(items))
}

/// DELETE /urls/{code} — xóa short URL thuộc về user đang đăng nhập
#[delete("/urls/{code}")]
pub async fn delete_url(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<SqlitePool>,
    jwt_secret: web::Data<String>,
) -> Result<impl Responder, actix_web::Error> {
    let claims = require_jwt(&req, &jwt_secret)?;
    let code = path.into_inner();

    let user_id = resolve_user_id(db.get_ref(), &claims.sub).await?
        .ok_or_else(|| json_err(StatusCode::NOT_FOUND, "User not found"))?;

    // Verify ownership before deleting
    let url_row = sqlx::query!(
        "SELECT id FROM urls WHERE short_code = ? AND user_id = ?",
        code,
        user_id
    )
    .fetch_optional(db.get_ref())
    .await
    .map_err(|_| json_err(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR))?
    .ok_or_else(|| json_err(StatusCode::NOT_FOUND, "Link not found or not owned by you"))?;

    // No ON DELETE CASCADE — delete clicks first
    sqlx::query!("DELETE FROM clicks WHERE url_id = ?", url_row.id)
        .execute(db.get_ref())
        .await
        .map_err(|_| json_err(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR))?;

    sqlx::query!("DELETE FROM urls WHERE id = ?", url_row.id)
        .execute(db.get_ref())
        .await
        .map_err(|_| json_err(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({"message": "Deleted"})))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};
    use sqlx::{Row, SqlitePool};

    /// Create an in-memory SQLite pool and run the schema migrations.
    async fn setup_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory DB");

        sqlx::query(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE urls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                short_code TEXT NOT NULL UNIQUE,
                original_url TEXT NOT NULL,
                user_id INTEGER REFERENCES users(id),
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE clicks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url_id INTEGER NOT NULL REFERENCES urls(id),
                clicked_at TEXT NOT NULL DEFAULT (datetime('now')),
                ip_address TEXT,
                user_agent TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    /// Build a valid JWT for testing.
    fn make_token(username: &str) -> String {
        use jsonwebtoken::{encode, EncodingKey, Header};

        let expiry = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::hours(1))
            .unwrap()
            .timestamp() as usize;

        let claims = Claims {
            sub: username.to_string(),
            exp: expiry,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"secret"),
        )
        .unwrap()
    }

    #[actix_web::test]
    async fn test_get_stats_not_found() {
        let pool = setup_db().await;
        let token = make_token("testuser");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool))
                .app_data(web::Data::new("secret".to_string()))
                .service(get_stats),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/stats/nonexistent")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[actix_web::test]
    async fn test_get_stats_no_auth() {
        let pool = setup_db().await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool))
                .app_data(web::Data::new("secret".to_string()))
                .service(get_stats),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/stats/anycode")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_web::test]
    async fn test_get_stats_returns_correct_data() {
        let pool = setup_db().await;

        // Seed a URL row
        sqlx::query(
            "INSERT INTO urls (short_code, original_url) VALUES ('abc123', 'https://example.com')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Seed two click rows
        let url_row = sqlx::query("SELECT id FROM urls WHERE short_code = 'abc123'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let url_id: i64 = url_row.get(0);

        for _ in 0..2 {
            sqlx::query(
                "INSERT INTO clicks (url_id, ip_address, user_agent) VALUES (?, '127.0.0.1', 'TestAgent')",
            )
            .bind(url_id)
            .execute(&pool)
            .await
            .unwrap();
        }

        let token = make_token("testuser");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool))
                .app_data(web::Data::new("secret".to_string()))
                .service(get_stats),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/stats/abc123")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["short_code"], "abc123");
        assert_eq!(body["original_url"], "https://example.com");
        assert_eq!(body["click_count"], 2);
        assert_eq!(body["clicks"].as_array().unwrap().len(), 2);
        assert_eq!(body["clicks"][0]["ip_address"], "127.0.0.1");
        assert_eq!(body["clicks"][0]["user_agent"], "TestAgent");
    }

    #[actix_web::test]
    async fn test_get_stats_zero_clicks() {
        let pool = setup_db().await;

        sqlx::query(
            "INSERT INTO urls (short_code, original_url) VALUES ('zzz999', 'https://rust-lang.org')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let token = make_token("testuser");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool))
                .app_data(web::Data::new("secret".to_string()))
                .service(get_stats),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/stats/zzz999")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["click_count"], 0);
        assert_eq!(body["clicks"].as_array().unwrap().len(), 0);
    }
}
