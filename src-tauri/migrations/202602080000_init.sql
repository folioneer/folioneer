-- Account tables
CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    update_frequency TEXT NOT NULL,
    is_deleted INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_name_active 
ON accounts(name) 
WHERE is_deleted = 0;

CREATE TABLE IF NOT EXISTS asset_accounts (
    account_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    average_price REAL NOT NULL,
    quantity REAL NOT NULL,
    PRIMARY KEY (account_id, asset_id),
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (asset_id) REFERENCES assets(id)
);

-- Asset tables
CREATE TABLE IF NOT EXISTS assets (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    reference TEXT NOT NULL,
    asset_class TEXT NOT NULL,
    category_id TEXT NOT NULL DEFAULT 'default-uncategorized',
    currency TEXT NOT NULL,
    risk_level INTEGER NOT NULL,
    is_deleted INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (category_id) REFERENCES categories(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_reference_active 
ON assets(reference) 
WHERE is_deleted = 0;

CREATE TABLE IF NOT EXISTS categories (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    is_deleted INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_categories_active 
ON categories(name) 
WHERE is_deleted = 0;

CREATE TABLE IF NOT EXISTS asset_prices (
    id TEXT PRIMARY KEY NOT NULL,
    asset_id TEXT NOT NULL,
    price REAL NOT NULL,
    date TEXT NOT NULL,
    FOREIGN KEY (asset_id) REFERENCES assets(id),
    UNIQUE (asset_id, date)
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_asset_prices_asset_date ON asset_prices(asset_id, date);
CREATE INDEX IF NOT EXISTS idx_asset_prices_date ON asset_prices(date);

INSERT INTO categories (id, name) 
VALUES ('default-uncategorized', 'Uncategorized')
ON CONFLICT(id) DO NOTHING;
