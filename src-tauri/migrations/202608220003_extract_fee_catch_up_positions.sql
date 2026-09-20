-- The fee catch-up cursor (FEE-043) becomes its own record, identified by the schedule's
-- (account, asset) and merged by maximum between devices (CFR-044), so the schedule itself
-- stays a user-owned record (CFR-016). Owning bounded context: account.
-- last_applied_period: ISO date of the most recent generated period boundary (FEE-043).
-- The three sync_* columns are the CFR-014 rank, as on every synced table (see M1).
CREATE TABLE IF NOT EXISTS fee_catch_up_positions (
    account_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    last_applied_period TEXT NOT NULL,
    sync_logical_timestamp TEXT,
    sync_origin TEXT,
    sync_device_id TEXT,
    PRIMARY KEY (account_id, asset_id),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE RESTRICT
);

-- The asset_id FK has no leftmost-column covering index (the PK starts with account_id),
-- so an explicit index keeps ON DELETE RESTRICT checks off a full scan.
CREATE INDEX IF NOT EXISTS idx_fee_catch_up_positions_asset_id ON fee_catch_up_positions (asset_id);

-- Copy every existing cursor. Parents (accounts, assets) already exist; FK checks are
-- deferred to commit so the copy is order-independent within the migration transaction.
PRAGMA defer_foreign_keys = ON;

INSERT INTO fee_catch_up_positions (account_id, asset_id, last_applied_period)
SELECT account_id, asset_id, last_applied_period
FROM fee_schedules
WHERE last_applied_period IS NOT NULL;

-- Safe plain DROP: SQLite 3.46 (libsqlite3-sys 0.30.1) and the column carries no index,
-- constraint or foreign key.
ALTER TABLE fee_schedules DROP COLUMN last_applied_period;
