-- Multi-device sync schema (SYN spec, ADR-019). Owned by the `sync` bounded context.
-- Created here (PR-A) so the change recorder can exist before any device enables sync;
-- every table stays empty until `enable_sync` (SYN-010) — with no sync_device row the
-- recorder is a no-op.

-- This installation's membership (SYN-016/018/023/052/070/084). A singleton: id is always 1.
-- logical_clock: the Lamport counter behind every logical timestamp (CFR-010, SYN-025).
-- derived_key: the passphrase-derived encryption key, kept so automatic sync never prompts
--   (SYN-052); the passphrase itself is never stored.
-- portfolio_created_at: the folder header's creation mark this device follows (SYN-084).
CREATE TABLE IF NOT EXISTS sync_device (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    device_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    folder TEXT NOT NULL,
    joined_at TEXT NOT NULL,
    paused INTEGER NOT NULL DEFAULT 0,
    portfolio_created_at TEXT NOT NULL,
    logical_clock INTEGER NOT NULL DEFAULT 0,
    derived_key BLOB NOT NULL,
    data_format_version INTEGER NOT NULL
);

-- One recorded modification of one record (SYN-020). sequence is this device's own strictly
-- increasing position (SYN-025); logical_timestamp / origin / based_on per CFR-010/011/016;
-- content: the full record state after the change as JSON, NULL for a removal (SYN-024);
-- published: 0 until the change has been written into a segment (SYN-031/067).
CREATE TABLE IF NOT EXISTS changes (
    device_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    logical_timestamp TEXT NOT NULL,
    based_on TEXT,
    record_kind TEXT NOT NULL,
    record_identity TEXT NOT NULL,
    operation TEXT NOT NULL,
    origin TEXT NOT NULL,
    content TEXT,
    published INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (device_id, sequence)
);

CREATE INDEX IF NOT EXISTS idx_changes_published ON changes (published);
CREATE INDEX IF NOT EXISTS idx_changes_record ON changes (record_kind, record_identity);

-- What a removal leaves behind (CFR-015): stands in for the removed record when a later or
-- earlier change to it arrives; kept permanently, never pruned.
CREATE TABLE IF NOT EXISTS tombstones (
    record_kind TEXT NOT NULL,
    record_identity TEXT NOT NULL,
    logical_timestamp TEXT NOT NULL,
    origin TEXT NOT NULL,
    removed_by TEXT NOT NULL,
    PRIMARY KEY (record_kind, record_identity)
);

-- How far this device has taken in each other device's changes (SYN-033/037).
CREATE TABLE IF NOT EXISTS sync_cursors (
    device_id TEXT PRIMARY KEY NOT NULL,
    applied_through INTEGER NOT NULL,
    last_applied_at TEXT
);

-- A received change waiting for a record or a based-on state this device has not received
-- yet (SYN-041, CFR-011/031). payload: the change as received, JSON.
CREATE TABLE IF NOT EXISTS held_back_changes (
    id TEXT PRIMARY KEY NOT NULL,
    origin_device_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    payload TEXT NOT NULL,
    waiting_kind TEXT NOT NULL,
    waiting_identity TEXT NOT NULL,
    held_since TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_held_back_waiting ON held_back_changes (waiting_kind, waiting_identity);

-- A persisted conflict notice (SYN-066, CFR-060), shown until the user dismisses it.
CREATE TABLE IF NOT EXISTS conflict_notices (
    notice_id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    record_kind TEXT NOT NULL,
    record_identity TEXT NOT NULL,
    record_label TEXT NOT NULL,
    other_device_id TEXT NOT NULL,
    other_device_name TEXT NOT NULL,
    raised_at TEXT NOT NULL,
    dismissed INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_conflict_notices_dismissed ON conflict_notices (dismissed);
