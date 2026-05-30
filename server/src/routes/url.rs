use actix_web::HttpMessage;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use chrono::NaiveDateTime;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::db::analytics;
use crate::models::Claims;

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
    pub total_clicks: i64,
    pub clicks_per_day: Vec<analytics::DailyClick>,
    pub recent_clicks: Vec<ClickRecord>,
}

fn claims_from_extensions(req: &HttpRequest) -> Result<Claims, actix_web::Error> {
    req.extensions()
        .get::<Claims>()
        .cloned()
        .ok_or_else(|| {
            actix_web::error::ErrorUnauthorized(
                serde_json::json!({"error": "Unauthorized"}).to_string(),
            )
        })
}

fn require_jwt(req: &HttpRequest, jwt_secret: &str) -> Result<Claims, actix_web::Error> {
    let header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            actix_web::error::ErrorUnauthorized(
                serde_json::json!({"error": "Missing Authorization header"}).to_string(),
            )
        })?;

    let token = header.strip_prefix("Bearer ").ok_or_else(|| {
        actix_web::error::ErrorUnauthorized(
            serde_json::json!({"error": "Invalid Authorization header format"}).to_string(),
        )
    })?;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| {
        actix_web::error::ErrorUnauthorized(
            serde_json::json!({"error": "Invalid or expired token"}).to_string(),
        )
    })?;

    Ok(token_data.claims)
}

#[post("/shorten")]
pub async fn shorten(
    req: HttpRequest,
    body: web::Json<ShortenRequest>,
    db: web::Data<SqlitePool>,
    base_url: web::Data<String>,
) -> Result<impl Responder, actix_web::Error> {
    let claims = claims_from_extensions(&req)?;

    if !body.original_url.starts_with("http://") && !body.original_url.starts_with("https://") {
        return Err(actix_web::error::ErrorBadRequest(
            serde_json::json!({"error": "original_url must start with http:// or https://"})
                .to_string(),
        ));
    }

    let user_row = sqlx::query("SELECT id FROM users WHERE username = ?")
        .bind(&claims.sub)
        .fetch_optional(db.get_ref())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let user_id: Option<i64> = user_row.map(|r| r.get("id"));

    let code = loop {
        let candidate = nanoid::nanoid!(6);
        let exists = sqlx::query("SELECT id FROM urls WHERE short_code = ?")
            .bind(&candidate)
            .fetch_optional(db.get_ref())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
        if exists.is_none() {
            break candidate;
        }
    };

    sqlx::query("INSERT INTO urls (short_code, original_url, user_id) VALUES (?, ?, ?)")
        .bind(&code)
        .bind(&body.original_url)
        .bind(user_id)
        .execute(db.get_ref())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(ShortenResponse {
        short_url: format!("{}/{}", base_url.as_ref(), code),
        short_code: code,
    }))
}

#[get("/{code}")]
pub async fn redirect(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<SqlitePool>,
) -> Result<impl Responder, actix_web::Error> {
    let code = path.into_inner();

    if code == "api" || code == "health" {
        return Err(actix_web::error::ErrorNotFound("not found"));
    }

    let row = sqlx::query("SELECT id, original_url FROM urls WHERE short_code = ?")
        .bind(&code)
        .fetch_optional(db.get_ref())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| {
            actix_web::error::ErrorNotFound(
                serde_json::json!({"error": "Short URL not found"}).to_string(),
            )
        })?;

    let url_id: i64 = row.get("id");
    let original_url: String = row.get("original_url");

    let ip = req.connection_info().peer_addr().map(str::to_string);
    let ua = req
        .headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    sqlx::query("INSERT INTO clicks (url_id, ip_address, user_agent) VALUES (?, ?, ?)")
        .bind(url_id)
        .bind(ip)
        .bind(ua)
        .execute(db.get_ref())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::MovedPermanently()
        .insert_header(("Location", original_url))
        .finish())
}

#[get("/stats/{code}")]
pub async fn get_stats(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<SqlitePool>,
    jwt_secret: web::Data<String>,
) -> Result<impl Responder, actix_web::Error> {
    let claims = claims_from_extensions(&req)
        .or_else(|_| require_jwt(&req, &jwt_secret))?;

    let code = path.into_inner();

    let stats = analytics::get_url_stats(db.get_ref(), &code, &claims.sub)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| {
            actix_web::error::ErrorNotFound(
                serde_json::json!({"error": "Short URL not found"}).to_string(),
            )
        })?;

    let url_row = sqlx::query("SELECT id FROM urls WHERE short_code = ?")
        .bind(&code)
        .fetch_optional(db.get_ref())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| {
            actix_web::error::ErrorNotFound(
                serde_json::json!({"error": "Short URL not found"}).to_string(),
            )
        })?;

    let url_id: i64 = url_row.get("id");

    let clicks_per_day = analytics::get_daily_clicks(db.get_ref(), url_id)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let click_rows = sqlx::query(
        "SELECT clicked_at, ip_address, user_agent \
         FROM clicks WHERE url_id = ? ORDER BY clicked_at DESC LIMIT 50",
    )
    .bind(url_id)
    .fetch_all(db.get_ref())
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;

    let recent_clicks: Vec<ClickRecord> = click_rows
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
        total_clicks: stats.total_clicks,
        clicks_per_day,
        recent_clicks,
    }))
}
