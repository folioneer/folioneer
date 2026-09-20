-- Recurring per-(account, asset) management-fee schedule (FEE-030/031). At most one per pair.
-- annual_rate_micros: micro-percent per year (1% = 1_000_000, FEE-032 / ADR-001).
-- frequency: 'Monthly' | 'Quarterly' | 'Annually' (FEE-034).
-- active: 1 while generating, 0 while paused (FEE-061).
-- last_applied_period: ISO date of the most recent generated period boundary — the catch-up
--   cursor (FEE-043); NULL until the first deduction is generated.
-- The UNIQUE(account_id, asset_id) index also serves the account_id FK lookup (leftmost column).
CREATE TABLE IF NOT EXISTS fee_schedules (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    annual_rate_micros INTEGER NOT NULL,
    frequency TEXT NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT,
    active INTEGER NOT NULL DEFAULT 1,
    last_applied_period TEXT,
    UNIQUE (account_id, asset_id),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE RESTRICT
);

-- The asset_id FK has no leftmost-column covering index (the UNIQUE starts with account_id),
-- so an explicit index keeps ON DELETE RESTRICT checks and asset-side lookups off a full scan.
CREATE INDEX IF NOT EXISTS idx_fee_schedules_asset_id ON fee_schedules (asset_id);
