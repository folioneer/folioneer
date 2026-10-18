-- Price fetch failures (MKT-209) — owned by use_cases/shared/price_fetch_failures.rs.
-- Device-local, outside the synced data (SYN-023): for each asset the last fetch on this
-- installation could not price, why and when. A fetch that prices the asset deletes its
-- row; no row means no failure to tell. An asset removed takes its row with it. The key
-- is the asset itself: one row per asset, nothing to generate.
CREATE TABLE IF NOT EXISTS price_fetch_failures (
    asset_id TEXT PRIMARY KEY NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    reason TEXT NOT NULL CHECK (reason IN ('NoData', 'ProviderUnreachable', 'NotSaved')),
    failed_at TEXT NOT NULL
);
