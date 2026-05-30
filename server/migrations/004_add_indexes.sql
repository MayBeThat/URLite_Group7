CREATE INDEX IF NOT EXISTS idx_urls_short_code   ON urls(short_code);
CREATE INDEX IF NOT EXISTS idx_urls_user_id      ON urls(user_id);
CREATE INDEX IF NOT EXISTS idx_clicks_url_id     ON clicks(url_id);
CREATE INDEX IF NOT EXISTS idx_clicks_clicked_at ON clicks(clicked_at);
CREATE INDEX IF NOT EXISTS idx_clicks_url_date   ON clicks(url_id, clicked_at);
