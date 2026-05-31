use actix_web::{post, web, HttpResponse};
use bcrypt::{hash, verify, DEFAULT_COST};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::error::{AppError, AppResult};
use crate::models::{Claims, JwtSecret};

const ERR_USERNAME_TAKEN: &str = "Username already taken";
const ERR_INVALID_CREDENTIALS: &str = "Invalid credentials";

#[derive(Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub token: String,
}

#[post("/auth/register")]
pub async fn register(
    body: web::Json<AuthRequest>,
    db: web::Data<SqlitePool>,
) -> AppResult<HttpResponse> {
    let password_hash = hash(&body.password, DEFAULT_COST)?;

    sqlx::query("INSERT INTO users (username, password_hash) VALUES (?, ?)")
        .bind(&body.username)
        .bind(&password_hash)
        .execute(db.get_ref())
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(d) if d.is_unique_violation() => {
                AppError::Conflict(ERR_USERNAME_TAKEN)
            }
            other => AppError::Internal(other.to_string()),
        })?;

    Ok(HttpResponse::Created()
        .json(serde_json::json!({"message": "User registered successfully"})))
}

#[post("/auth/login")]
pub async fn login(
    body: web::Json<AuthRequest>,
    db: web::Data<SqlitePool>,
    jwt_secret: web::Data<JwtSecret>,
) -> AppResult<HttpResponse> {
    let row = sqlx::query("SELECT password_hash FROM users WHERE username = ?")
        .bind(&body.username)
        .fetch_optional(db.get_ref())
        .await?
        .ok_or(AppError::Unauthorized(ERR_INVALID_CREDENTIALS))?;

    let password_hash: String = row.get("password_hash");

    if !verify(&body.password, &password_hash)? {
        return Err(AppError::Unauthorized(ERR_INVALID_CREDENTIALS));
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
        &EncodingKey::from_secret(jwt_secret.0.as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(HttpResponse::Ok().json(TokenResponse { token }))
}
