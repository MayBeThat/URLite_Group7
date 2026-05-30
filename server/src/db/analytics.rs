use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Serialize)]
pub struct DailyClick {
    pub day: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct UrlStats {
    pub short_code: String,
    pub original_url: String,
    pub total_clicks: i64,
    pub created_at: NaiveDateTime,
}

pub async fn get_daily_clicks(
    pool: &SqlitePool,
    url_id: i64,
) -> Result<Vec<DailyClick>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            strftime('%Y-%m-%d', clicked_at) AS day,
            COUNT(*) AS count
        FROM clicks
        WHERE url_id = ?
        GROUP BY strftime('%Y-%m-%d', clicked_at)
        ORDER BY 1 ASC
        "#,
    )
    .bind(url_id)
    .fetch_all(pool)
    .await?;

    use sqlx::Row;
    Ok(rows
        .into_iter()
        .map(|r| DailyClick {
            day: r.get("day"),
            count: r.get("count"),
        })
        .collect())
}

pub async fn get_url_stats(
    pool: &SqlitePool,
    short_code: &str,
    username: &str,
) -> Result<Option<UrlStats>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
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

    use sqlx::Row;
    Ok(row.map(|r| UrlStats {
        short_code: r.get("short_code"),
        original_url: r.get("original_url"),
        total_clicks: r.get("total_clicks"),
        created_at: r.get("created_at"),
    }))
}
