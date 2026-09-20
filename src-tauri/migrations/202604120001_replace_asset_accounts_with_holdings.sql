-- Replace asset_accounts (REAL fields) with holdings (i64 micro-units) — ADR-002
DROP TABLE IF EXISTS asset_accounts;

CREATE TABLE IF NOT EXISTS holdings (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 0,
    average_price INTEGER NOT NULL DEFAULT 0,
    UNIQUE (account_id, asset_id),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE RESTRICT
);
