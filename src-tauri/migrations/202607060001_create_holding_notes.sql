-- Per-(account, asset) free-text holding note; at most one per pair (HNO-010).
-- text: required, trimmed, 1-500 chars (HNO-011).
-- threshold_price: nominal share price in asset-currency micros (HNO-031); alarm part 1.
-- threshold_direction: 'Below' | 'Above'; alarm part 2 — both alarm fields or neither (HNO-011).
-- Both FKs cascade: removing the account or the asset removes its notes (HNO-010).
CREATE TABLE IF NOT EXISTS holding_notes (
    account_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    text TEXT NOT NULL,
    threshold_price INTEGER,
    threshold_direction TEXT,
    PRIMARY KEY (account_id, asset_id),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

-- The asset_id FK has no leftmost-column covering index (the PK starts with account_id),
-- so an explicit index keeps ON DELETE CASCADE checks and asset-side lookups off a full scan.
CREATE INDEX IF NOT EXISTS idx_holding_notes_asset_id ON holding_notes (asset_id);
