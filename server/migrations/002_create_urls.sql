CREATE TABLE IF NOT EXISTS urls (
    id           INTEGER     PRIMARY KEY AUTOINCREMENT,
    user_id      INTEGER     REFERENCES users(id) ON DELETE CASCADE,
    short_code   TEXT        NOT NULL UNIQUE,
    original_url TEXT        NOT NULL,
    created_at   TEXT        NOT NULL DEFAULT (datetime('now'))
);
