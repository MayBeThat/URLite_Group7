use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde_json::json;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    NotFound(&'static str),
    Unauthorized(&'static str),
    BadRequest(&'static str),
    Conflict(&'static str),
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotFound(m)
            | AppError::Unauthorized(m)
            | AppError::BadRequest(m)
            | AppError::Conflict(m) => f.write_str(m),
            AppError::Internal(m) => f.write_str(m),
        }
    }
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let msg = match self {
            AppError::Internal(_) => "Internal server error",
            AppError::NotFound(m)
            | AppError::Unauthorized(m)
            | AppError::BadRequest(m)
            | AppError::Conflict(m) => m,
        };
        HttpResponse::build(self.status_code()).json(json!({ "error": msg }))
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                AppError::Conflict("Resource already exists")
            }
            other => AppError::Internal(other.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl From<bcrypt::BcryptError> for AppError {
    fn from(e: bcrypt::BcryptError) -> Self {
        AppError::Internal(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
