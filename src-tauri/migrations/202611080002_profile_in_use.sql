-- PFL-012 — the profile in use: this computer's, kept across restarts and never synced
-- (SYN-023): no change is recorded for it. One row at most; none means the default profile.
CREATE TABLE IF NOT EXISTS profile_in_use (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    profile_id TEXT NOT NULL
);
