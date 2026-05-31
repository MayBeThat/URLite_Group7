use actix_files::NamedFile;
use actix_web::{delete, get, post, web, HttpMessage, HttpRequest, HttpResponse};
use chrono::NaiveDateTime;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::db::{analytics, clicks};
use crate::error::{AppError, AppResult};
use crate::models::{BaseUrl, Claims, FrontendDir, JwtSecret};

const ERR_NOT_FOUND: &str = "Short URL not found";
const ERR_INVALID_URL: &str = "original_url must start with http:// or https://";
const ERR_MISSING_AUTH: &str = "Missing Authorization header";
const ERR_INVALID_AUTH: &str = "Invalid Authorization header format";
const ERR_INVALID_TOKEN: &str = "Invalid or expired token";
const ERR_UNAUTHORIZED: &str = "Unauthorized";

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
    pub clicked_at: NaiveDateTime,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub short_code: String,
    pub original_url: String,
    pub created_at: NaiveDateTime,
    pub click_count: i64,
    pub clicks: Vec<ClickRecord>,
}

#[derive(Serialize)]
pub struct UrlListItem {
    pub short_code: String,
    pub short_url: String,
    pub original_url: String,
    pub created_at: String,
    pub click_count: i64,
}

fn claims_from_extensions(req: &HttpRequest) -> AppResult<Claims> {
    req.extensions()
        .get::<Claims>()
        .cloned()
        .ok_or(AppError::Unauthorized(ERR_UNAUTHORIZED))
}

fn require_jwt(req: &HttpRequest, jwt_secret: &str) -> AppResult<Claims> {
    let header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized(ERR_MISSING_AUTH))?;

    let token = header
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized(ERR_INVALID_AUTH))?;

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|d| d.claims)
    .map_err(|_| AppError::Unauthorized(ERR_INVALID_TOKEN))
}

#[post("")]
pub async fn shorten(
    req: HttpRequest,
    body: web::Json<ShortenRequest>,
    db: web::Data<SqlitePool>,
    base_url: web::Data<BaseUrl>,
) -> AppResult<HttpResponse> {
    let claims = claims_from_extensions(&req)?;

    if !body.original_url.starts_with("http://") && !body.original_url.starts_with("https://") {
        return Err(AppError::BadRequest(ERR_INVALID_URL));
    }

    let user_id: Option<i64> = sqlx::query("SELECT id FROM users WHERE username = ?")
        .bind(&claims.sub)
        .fetch_optional(db.get_ref())
        .await?
        .map(|r| r.get("id"));

    let code = loop {
        let candidate = nanoid::nanoid!(6);
        let exists = sqlx::query("SELECT id FROM urls WHERE short_code = ?")
            .bind(&candidate)
            .fetch_optional(db.get_ref())
            .await?;
        if exists.is_none() {
            break candidate;
        }
    };

    sqlx::query("INSERT INTO urls (short_code, original_url, user_id) VALUES (?, ?, ?)")
        .bind(&code)
        .bind(&body.original_url)
        .bind(user_id)
        .execute(db.get_ref())
        .await?;

    Ok(HttpResponse::Ok().json(ShortenResponse {
        short_url: format!("{}/{}", base_url.0, code),
        short_code: code,
    }))
}

#[get("/{code}")]
pub async fn redirect(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<SqlitePool>,
    frontend_dir: web::Data<FrontendDir>,
) -> AppResult<HttpResponse> {
    let code = path.into_inner();

    if code == "api" || code == "health" {
        return Err(AppError::NotFound(ERR_NOT_FOUND));
    }

    let row = sqlx::query("SELECT id, original_url FROM urls WHERE short_code = ?")
        .bind(&code)
        .fetch_optional(db.get_ref())
        .await?;

    if let Some(row) = row {
        let url_id: i64 = row.get("id");
        let original_url: String = row.get("original_url");

        clicks::log_click(db.get_ref(), url_id, &req).await?;

        return Ok(HttpResponse::MovedPermanently()
            .insert_header(("Location", original_url))
            .finish());
    }

    let file_path = std::path::Path::new(&frontend_dir.0).join(&code);
    if file_path.is_file() {
        return Ok(NamedFile::open(file_path)?.into_response(&req));
    }

    Err(AppError::NotFound(ERR_NOT_FOUND))
}

#[get("/stats/{code}")]
pub async fn get_stats(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<SqlitePool>,
    jwt_secret: web::Data<JwtSecret>,
) -> AppResult<HttpResponse> {
    let claims = claims_from_extensions(&req).or_else(|_| require_jwt(&req, &jwt_secret.0))?;
    let code = path.into_inner();

    let stats = analytics::get_url_stats(db.get_ref(), &code, &claims.sub)
        .await?
        .ok_or(AppError::NotFound(ERR_NOT_FOUND))?;

    let click_rows = sqlx::query(
        "SELECT clicked_at, ip_address, user_agent \
         FROM clicks WHERE url_id = ? ORDER BY clicked_at DESC LIMIT 50",
    )
    .bind(stats.id)
    .fetch_all(db.get_ref())
    .await?;

    let clicks: Vec<ClickRecord> = click_rows
        .into_iter()
        .map(|r| ClickRecord {
            clicked_at: r.get("clicked_at"),
            ip_address: r.get("ip_address"),
            user_agent: r.get("user_agent"),
        })
        .collect();

    Ok(HttpResponse::Ok().json(StatsResponse {
        short_code: stats.short_code,
        original_url: stats.original_url,
        created_at: stats.created_at,
        click_count: stats.total_clicks,
        clicks,
    }))
}

#[get("")]
pub async fn list_urls(
    req: HttpRequest,
    db: web::Data<SqlitePool>,
    base_url: web::Data<BaseUrl>,
) -> AppResult<HttpResponse> {
    let claims = claims_from_extensions(&req)?;

    let rows = sqlx::query(
        r#"
        SELECT u.short_code, u.original_url, u.created_at,
               COUNT(c.id) AS click_count
        FROM urls u
        INNER JOIN users us ON us.id = u.user_id
        LEFT JOIN clicks c ON c.url_id = u.id
        WHERE us.username = ?
        GROUP BY u.id
        ORDER BY u.created_at DESC
        "#,
    )
    .bind(&claims.sub)
    .fetch_all(db.get_ref())
    .await?;

    let items: Vec<UrlListItem> = rows
        .into_iter()
        .map(|r| {
            let code: String = r.get("short_code");
            UrlListItem {
                short_url: format!("{}/{}", base_url.0, code),
                short_code: code,
                original_url: r.get("original_url"),
                created_at: r.get("created_at"),
                click_count: r.get("click_count"),
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(items))
}

#[delete("/{code}")]
pub async fn delete_url(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<SqlitePool>,
) -> AppResult<HttpResponse> {
    let claims = claims_from_extensions(&req)?;
    let code = path.into_inner();

    let result = sqlx::query(
        r#"
        DELETE FROM urls
        WHERE short_code = ?
          AND user_id IN (SELECT id FROM users WHERE username = ?)
        "#,
    )
    .bind(&code)
    .bind(&claims.sub)
    .execute(db.get_ref())
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(ERR_NOT_FOUND));
    }

    Ok(HttpResponse::NoContent().finish())
}
