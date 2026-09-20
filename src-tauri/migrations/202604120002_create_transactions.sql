-- Create transactions table (TRX feature)
CREATE TABLE IF NOT EXISTS transactions (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    transaction_type TEXT NOT NULL DEFAULT 'Purchase',
    date TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    unit_price INTEGER NOT NULL,
    exchange_rate INTEGER NOT NULL,
    fees INTEGER NOT NULL DEFAULT 0,
    total_amount INTEGER NOT NULL,
    note TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE RESTRICT
);

-- Composite index for chronological queries per (account, asset) pair (TRX-036)
CREATE INDEX IF NOT EXISTS idx_transactions_account_asset
    ON transactions (account_id, asset_id, date);
