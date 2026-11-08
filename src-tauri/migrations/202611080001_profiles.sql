-- PFL-010/011/013 — profiles, each account in one. The default profile has a fixed identity,
-- the same on every device, and no name until the owner names it; every account that existed
-- before belongs to it. Synced (PFL-060; data format version 17).
CREATE TABLE IF NOT EXISTS profiles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT,
    sync_logical_timestamp TEXT,
    sync_origin TEXT,
    sync_device_id TEXT
);
INSERT OR IGNORE INTO profiles (id, name) VALUES ('default-profile', NULL);
ALTER TABLE accounts ADD COLUMN profile_id TEXT NOT NULL DEFAULT 'default-profile';
CREATE INDEX IF NOT EXISTS accounts_profile_id ON accounts (profile_id);
