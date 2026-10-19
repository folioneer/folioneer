-- MKT-230 — the splits a price provider reports for an asset, one row per (asset, date),
-- synced as an observation (SYN-021, CFR-050). They change no holding, price or transaction.
CREATE TABLE IF NOT EXISTS reported_splits (
    asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    new INTEGER NOT NULL CHECK (new > 0),
    old INTEGER NOT NULL CHECK (old > 0),
    sync_logical_timestamp TEXT,
    sync_origin TEXT,
    sync_device_id TEXT,
    PRIMARY KEY (asset_id, date)
);
