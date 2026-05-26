use actix_web::{post, web, HttpResponse, Responder};
use bcrypt::{hash, verify, DEFAULT_COST};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::models::Claims;

#[derive(Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub token: String,
}

/// POST /auth/register
#[post("/auth/register")]
pub async fn register(
    body: web::Json<AuthRequest>,
    db: web::Data<SqlitePool>,
) -> Result<impl Responder, actix_web::Error> {
    let password_hash = hash(&body.password, DEFAULT_COST)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    sqlx::query!(
        "INSERT INTO users (username, password_hash) VALUES (?, ?)",
        body.username,
        password_hash
    )
    .execute(db.get_ref())
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            actix_web::error::ErrorConflict(
                serde_json::json!({"error": "Username already taken"}).to_string(),
            )
        }
        other => actix_web::error::ErrorInternalServerError(other),
    })?;

    Ok(HttpResponse::Created().json(serde_json::json!({"message": "User registered successfully"})))
}

/// POST /auth/login
#[post("/auth/login")]
pub async fn login(
    body: web::Json<AuthRequest>,
    db: web::Data<SqlitePool>,
    jwt_secret: web::Data<String>,
) -> Result<impl Responder, actix_web::Error> {
    let row = sqlx::query!(
        "SELECT password_hash FROM users WHERE username = ?",
        body.username
    )
    .fetch_optional(db.get_ref())
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?
    .ok_or_else(|| {
        actix_web::error::ErrorUnauthorized(
            serde_json::json!({"error": "Invalid credentials"}).to_string(),
        )
    })?;

    let valid = verify(&body.password, &row.password_hash)
        .map_err(actix_web::error::ErrorInternalServerError)?;
    if !valid {
        return Err(actix_web::error::ErrorUnauthorized(
            serde_json::json!({"error": "Invalid credentials"}).to_string(),
        ));
    }

    let expiry = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: body.username.clone(),
        exp: expiry,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(TokenResponse { token }))
}
