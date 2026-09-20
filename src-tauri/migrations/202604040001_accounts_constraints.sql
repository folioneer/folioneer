-- R3: Replace case-sensitive unique index with case-insensitive one (LOWER(name))
-- R5: Switch accounts to hard-delete; asset_accounts gets ON DELETE CASCADE
DROP INDEX IF EXISTS idx_accounts_name_active;

CREATE UNIQUE INDEX idx_accounts_name_lower ON accounts(LOWER(name));

-- Recreate asset_accounts with ON DELETE CASCADE on account_id
-- SQLite does not support ALTER TABLE ADD CONSTRAINT
CREATE TABLE asset_accounts_new (
    account_id TEXT NOT NULL,
    asset_id   TEXT NOT NULL,
    average_price REAL NOT NULL,
    quantity      REAL NOT NULL,
    PRIMARY KEY (account_id, asset_id),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id)   REFERENCES assets(id)
);

INSERT INTO asset_accounts_new SELECT * FROM asset_accounts;

DROP TABLE asset_accounts;

ALTER TABLE asset_accounts_new RENAME TO asset_accounts;
