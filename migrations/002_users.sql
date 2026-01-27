-- Users table
CREATE TABLE users (
    username TEXT PRIMARY KEY,
    is_admin INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_users_is_admin ON users(is_admin);
