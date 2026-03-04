CREATE TABLE users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER))
);

CREATE TABLE remind_sessions (
    id VARCHAR(128) NOT NULL PRIMARY KEY,
    expires BIGINT NULL,
    session TEXT NOT NULL
);
