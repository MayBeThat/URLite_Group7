CREATE TABLE IF NOT EXISTS clicks (
    id           INTEGER     PRIMARY KEY AUTOINCREMENT,
    url_id       INTEGER     NOT NULL REFERENCES urls(id) ON DELETE CASCADE,
    clicked_at   TEXT        NOT NULL DEFAULT (datetime('now')),
    ip_address   TEXT,
    user_agent   TEXT
);
