-- Standalone FK indexes missing from earlier migrations (reviewer-sql finding)
-- asset_prices.asset_id is already the leftmost PK prefix — no additional index needed.
-- holdings.account_id is covered by UNIQUE(account_id, asset_id) leftmost prefix.
-- Existing migrations cannot be edited without breaking SQLx hash checks for installed users.

-- assets.category_id: enables JOIN/filter on category without full table scan
CREATE INDEX IF NOT EXISTS idx_assets_category_id ON assets (category_id);

-- holdings.asset_id: enables cross-account asset queries (UNIQUE covers account_id prefix only)
CREATE INDEX IF NOT EXISTS idx_holdings_asset_id ON holdings (asset_id);

-- transactions.asset_id: enables asset-wide history queries (composite covers account_id prefix only)
CREATE INDEX IF NOT EXISTS idx_transactions_asset_id ON transactions (asset_id);
