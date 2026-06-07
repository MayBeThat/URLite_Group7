use actix_files::NamedFile; // phục vụ một file tĩnh (dùng cho fallback SPA)
use actix_web::{delete, get, post, web, HttpMessage, HttpRequest, HttpResponse}; // macro route + tiện ích
use chrono::NaiveDateTime;  // kiểu ngày-giờ cho cột clicked_at/created_at
use jsonwebtoken::{decode, DecodingKey, Validation}; // dùng để xác minh JWT thủ công ở /stats
use serde::{Deserialize, Serialize}; // JSON <-> struct
use sqlx::{Row, SqlitePool};

use crate::db::{analytics, clicks}; // 2 module DB ta đã chú thích trước đó
use crate::error::{AppError, AppResult};
use crate::models::{BaseUrl, Claims, FrontendDir, JwtSecret};

// Gom các thông điệp lỗi dạng hằng số.
const ERR_NOT_FOUND: &str = "Short URL not found";
const ERR_INVALID_URL: &str = "original_url must start with http:// or https://";
const ERR_MISSING_AUTH: &str = "Missing Authorization header";
const ERR_INVALID_AUTH: &str = "Invalid Authorization header format";
const ERR_INVALID_TOKEN: &str = "Invalid or expired token";
const ERR_UNAUTHORIZED: &str = "Unauthorized";

// Body của request tạo link rút gọn: { "original_url": "..." }
#[derive(Deserialize)]
pub struct ShortenRequest {
    pub original_url: String,
}

// Phản hồi khi tạo link thành công.
#[derive(Serialize)]
pub struct ShortenResponse {
    pub short_code: String, // mã rút gọn
    pub short_url: String,  // URL đầy đủ để chia sẻ
}

// Một dòng click trong phần thống kê.
#[derive(Serialize)]
pub struct ClickRecord {
    pub clicked_at: NaiveDateTime,
    pub ip_address: Option<String>, // Option vì có thể NULL trong DB
    pub user_agent: Option<String>,
}

// Phản hồi thống kê đầy đủ cho 1 short URL.
#[derive(Serialize)]
pub struct StatsResponse {
    pub short_code: String,
    pub original_url: String,
    pub created_at: NaiveDateTime,
    pub click_count: i64,
    pub clicks: Vec<ClickRecord>, // Vec = danh sách động các click gần đây
}

// Một mục trong danh sách URL của người dùng (GET /urls).
#[derive(Serialize)]
pub struct UrlListItem {
    pub short_code: String,
    pub short_url: String,
    pub original_url: String,
    pub created_at: String,
    pub click_count: i64,
}

/// Hàm phụ: lấy Claims mà middleware đã chèn vào request (cho route có middleware bảo vệ).
fn claims_from_extensions(req: &HttpRequest) -> AppResult<Claims> {
    req.extensions()           // Khi Request đi qua một tầng bảo mật (Middleware xác thực), Middleware đã giải mã Token JWT, lấy ra thông tin User (Claims) rồi nhét tạm vào một cái "ngăn kéo bí mật" đính kèm theo Request gọi là extensions.
        .get::<Claims>()       // Hàm này thò tay vào ngăn kéo để tìm món đồ có kiểu dữ liệu là Claims
        .cloned()              // Vì Actix-web chỉ cho bạn xem (&Claims) chứ không cho lấy đi, bạn phải nhân bản sao chép nó ra một bản mới (cloned) để sở hữu hoàn toàn giá trị đó.
        .ok_or(AppError::Unauthorized(ERR_UNAUTHORIZED)) // Nếu ngăn kéo trống rỗng (User chưa đăng nhập hoặc token lỗi), ngay lập tức đá ra lỗi 401 Unauthorized.
}

// Hàm phụ: tự xác minh JWT từ header (dùng cho /stats vốn KHÔNG gắn middleware).
fn require_jwt(req: &HttpRequest, jwt_secret: &str) -> AppResult<Claims> {
    // Lấy chuỗi header "Authorization".
    let header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized(ERR_MISSING_AUTH))?; // thiếu header -> 401

    // Bỏ tiền tố "Bearer " để lấy phần token thực sự.
    let token = header
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized(ERR_INVALID_AUTH))?; // sai định dạng -> 401

    // Giải mã & xác minh token; trả về Claims nếu hợp lệ.
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(), // mặc định kiểm tra HS256 + hạn exp
    )
    .map(|d| d.claims)                                     // lấy phần claims
    .map_err(|_| AppError::Unauthorized(ERR_INVALID_TOKEN)) // token sai -> 401
}

// ===== POST /shorten — tạo link rút gọn (CẦN đăng nhập) =====
#[post("")] // đường dẫn rỗng vì route này nằm trong scope "/shorten" (xem main.rs)
pub async fn shorten(
    req: HttpRequest,
    body: web::Json<ShortenRequest>,
    db: web::Data<SqlitePool>,
    base_url: web::Data<BaseUrl>,
) -> AppResult<HttpResponse> {
    // Lấy user từ token (middleware đã xác minh trước đó).
    let claims = claims_from_extensions(&req)?;

    // Kiểm tra URL phải bắt đầu bằng http:// hoặc https:// (chặn dữ liệu rác).
    if !body.original_url.starts_with("http://") && !body.original_url.starts_with("https://") {
        return Err(AppError::BadRequest(ERR_INVALID_URL));
    }

    // Tra id của user theo username trong token. Kết quả Option<i64>.
    let user_id: Option<i64> = sqlx::query("SELECT id FROM users WHERE username = ?")
        .bind(&claims.sub)
        .fetch_optional(db.get_ref())
        .await?
        .map(|r| r.get("id"));

    // Sinh mã rút gọn duy nhất: lặp tới khi tạo được mã CHƯA tồn tại trong DB.
    let code = loop {
        let candidate = nanoid::nanoid!(6); // tạo chuỗi ngẫu nhiên 6 ký tự
        let exists = sqlx::query("SELECT id FROM urls WHERE short_code = ?")
            .bind(&candidate)
            .fetch_optional(db.get_ref())
            .await?;
        if exists.is_none() {
            break candidate; // không trùng -> dùng mã này và thoát vòng lặp
        }
        // nếu trùng (hiếm), vòng lặp chạy lại tạo mã khác
    };

    // Lưu link mới vào bảng urls.
    sqlx::query("INSERT INTO urls (short_code, original_url, user_id) VALUES (?, ?, ?)")
        .bind(&code)
        .bind(&body.original_url)
        .bind(user_id)
        .execute(db.get_ref())
        .await?;

    // Trả 200 OK kèm mã và URL đầy đủ (ghép base_url + code).
    Ok(HttpResponse::Ok().json(ShortenResponse {
        short_url: format!("{}/{}", base_url.0, code),
        short_code: code, // đặt sau cùng vì `code` bị "moved" vào đây
    }))
}

// ===== GET /{code} — chuyển hướng (CÔNG KHAI, không cần đăng nhập) =====
#[get("/{code}")]
pub async fn redirect(
    req: HttpRequest,
    path: web::Path<String>, // lấy phần {code} từ URL
    db: web::Data<SqlitePool>,
    frontend_dir: web::Data<FrontendDir>,
) -> AppResult<HttpResponse> {
    let code = path.into_inner(); // rút String ra khỏi web::Path

    // Chặn các từ dành riêng để chúng không bị hiểu nhầm là short code.
    if code == "api" || code == "health" {
        return Err(AppError::NotFound(ERR_NOT_FOUND));
    }

    // Tìm URL gốc theo short_code.
    let row = sqlx::query("SELECT id, original_url FROM urls WHERE short_code = ?")
        .bind(&code)
        .fetch_optional(db.get_ref())
        .await?;

    // Nếu tìm thấy -> ghi log click rồi chuyển hướng 301.
    if let Some(row) = row {
        let url_id: i64 = row.get("id");
        let original_url: String = row.get("original_url");

        clicks::log_click(db.get_ref(), url_id, &req).await?; // ghi lại lần click này

        return Ok(HttpResponse::MovedPermanently() // 301 Moved Permanently
            .insert_header(("Location", original_url)) // trình duyệt sẽ đi tới URL gốc
            .insert_header(("Cache-Control", "no-store")) // không cache để đếm đủ mọi lần click
            .finish());
    }

    // Không phải short code -> thử coi như yêu cầu một file tĩnh của frontend.
    let file_path = std::path::Path::new(&frontend_dir.0).join(&code);
    if file_path.is_file() {
        return Ok(NamedFile::open(file_path)?.into_response(&req));
    }

    // Không khớp gì cả -> 404.
    Err(AppError::NotFound(ERR_NOT_FOUND))
}

// ===== GET /stats/{code} — xem thống kê (CẦN đăng nhập) =====
#[get("/stats/{code}")]
pub async fn get_stats(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<SqlitePool>,
    jwt_secret: web::Data<JwtSecret>,
) -> AppResult<HttpResponse> {
    // Thử lấy Claims từ middleware; nếu không có thì tự xác minh token từ header.
    let claims = claims_from_extensions(&req).or_else(|_| require_jwt(&req, &jwt_secret.0))?;
    let code = path.into_inner();

    // Lấy thống kê tổng quan (và kiểm tra link thuộc về đúng user). None -> 404.
    let stats = analytics::get_url_stats(db.get_ref(), &code, &claims.sub)
        .await?
        .ok_or(AppError::NotFound(ERR_NOT_FOUND))?;

    // Lấy tối đa 50 click gần nhất của link để hiển thị chi tiết.
    let click_rows = sqlx::query(
        "SELECT clicked_at, ip_address, user_agent \
         FROM clicks WHERE url_id = ? ORDER BY clicked_at DESC LIMIT 50",
    )
    .bind(stats.id)
    .fetch_all(db.get_ref()) // fetch_all -> Vec các dòng
    .await?;

    // Chuyển từng dòng DB thành ClickRecord. .into_iter().map().collect() = biến đổi cả danh sách.
    let clicks: Vec<ClickRecord> = click_rows
        .into_iter()
        .map(|r| ClickRecord {
            clicked_at: r.get("clicked_at"),
            ip_address: r.get("ip_address"),
            user_agent: r.get("user_agent"),
        })
        .collect();

    // Gói tất cả vào StatsResponse và trả về JSON.
    Ok(HttpResponse::Ok().json(StatsResponse {
        short_code: stats.short_code,
        original_url: stats.original_url,
        created_at: stats.created_at,
        click_count: stats.total_clicks,
        clicks,
    }))
}

// ===== GET /urls — liệt kê mọi link của user (CẦN đăng nhập) =====
#[get("")] // nằm trong scope "/urls"
pub async fn list_urls(
    req: HttpRequest,
    db: web::Data<SqlitePool>,
    base_url: web::Data<BaseUrl>,
) -> AppResult<HttpResponse> {
    let claims = claims_from_extensions(&req)?;

    // Lấy mọi URL của user kèm số lượt click (LEFT JOIN để link 0 click vẫn hiện).
    let rows = sqlx::query(
        r#"
        SELECT u.short_code, u.original_url, u.created_at,
               COUNT(c.id) AS click_count
        FROM urls u
        INNER JOIN users us ON us.id = u.user_id   -- để lọc theo username
        LEFT JOIN clicks c ON c.url_id = u.id      -- đếm click, kể cả khi 0 clicks thì dùng LEFT JOIN
        WHERE us.username = ?
        GROUP BY u.id
        ORDER BY u.created_at DESC                  -- link mới nhất lên đầu
        "#,
    )
    .bind(&claims.sub)
    .fetch_all(db.get_ref())
    .await?;

    // Biến từng dòng thành UrlListItem, đồng thời ghép short_url đầy đủ.
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

    Ok(HttpResponse::Ok().json(items)) // trả về mảng JSON
}

// ===== DELETE /urls/{code} — xóa link (CẦN đăng nhập, chỉ chủ sở hữu) =====
#[delete("/{code}")]
pub async fn delete_url(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<SqlitePool>,
) -> AppResult<HttpResponse> {
    let claims = claims_from_extensions(&req)?;
    let code = path.into_inner();

    // Xóa link, nhưng chỉ khi nó thuộc về user hiện tại (kiểm tra qua subquery).
    // Đây là phần bảo vệ: user A không thể xóa link của user B.
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

    // rows_affected() = số dòng bị xóa. Bằng 0 nghĩa là không có link nào khớp -> 404.
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(ERR_NOT_FOUND));
    }

    // Xóa thành công -> 204 No Content (không cần trả body).
    Ok(HttpResponse::NoContent().finish())
}
