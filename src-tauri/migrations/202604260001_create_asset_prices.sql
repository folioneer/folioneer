CREATE TABLE asset_prices (
    asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    date     TEXT NOT NULL,
    price    INTEGER NOT NULL,
    PRIMARY KEY (asset_id, date)
);
