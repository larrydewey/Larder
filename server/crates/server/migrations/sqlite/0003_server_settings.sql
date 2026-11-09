-- Server-wide settings changed by admins in the app (for example the AI assistant).
-- Secrets inside values are encrypted (see src/secrets.rs).
CREATE TABLE server_settings (
    name TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    updated_by TEXT
);
