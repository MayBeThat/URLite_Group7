use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppResult;

#[derive(Debug, Serialize)]
pub struct UrlStats {
    pub id: i64,
    pub short_code: String,
    pub original_url: String,
    pub total_clicks: i64,
    pub created_at: NaiveDateTime,
}

pub async fn get_url_stats(
    pool: &SqlitePool,
    short_code: &str,
    username: &str,
) -> AppResult<Option<UrlStats>> {
    let row = sqlx::query(
        r#"
        SELECT
            u.id,
            u.short_code,
            u.original_url,
            COUNT(c.id) AS total_clicks,
            u.created_at
        FROM urls u
        LEFT JOIN clicks c ON c.url_id = u.id
        INNER JOIN users us ON us.id = u.user_id
        WHERE u.short_code = ?
          AND us.username = ?
        GROUP BY u.id
        "#,
    )
    .bind(short_code)
    .bind(username)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| UrlStats {
        id: r.get("id"),
        short_code: r.get("short_code"),
        original_url: r.get("original_url"),
        total_clicks: r.get("total_clicks"),
        created_at: r.get("created_at"),
    }))
}
