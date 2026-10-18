-- TODO-032 — portfolio-wide settings, one row per setting name, synced (SYN-021). The
-- reference currency is the first; a portfolio without a row takes the default (EUR).
CREATE TABLE IF NOT EXISTS portfolio_settings (
    name TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    sync_logical_timestamp TEXT,
    sync_origin TEXT,
    sync_device_id TEXT
);
