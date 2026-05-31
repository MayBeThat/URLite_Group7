use actix_web::HttpRequest;
use sqlx::SqlitePool;

use crate::error::AppResult;

pub async fn log_click(pool: &SqlitePool, url_id: i64, req: &HttpRequest) -> AppResult<()> {
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
        .execute(pool)
        .await?;

    Ok(())
}
