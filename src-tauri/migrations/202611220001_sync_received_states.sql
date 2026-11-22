-- CFR-011 (#925) — every state of a synced record this device received from another device,
-- applied, superseded, dropped or ignored: a change based on one of them is decided, never held
-- back waiting for a state that will not be applied. Local only: never synced, emptied when the
-- device leaves sync (SYN-082).
-- One row per received state: kind, identity and timestamp together form the key.
CREATE TABLE IF NOT EXISTS sync_received_states (
    record_kind TEXT NOT NULL,
    record_identity TEXT NOT NULL,
    logical_timestamp TEXT NOT NULL,
    PRIMARY KEY (record_kind, record_identity, logical_timestamp)
);
