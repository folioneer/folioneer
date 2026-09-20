-- AssetPrice is removed as an orphaned feature.
-- Market price tracking will be redesigned as a dedicated feature.
DROP INDEX IF EXISTS idx_asset_prices_asset_date;
DROP INDEX IF EXISTS idx_asset_prices_date;
DROP TABLE IF EXISTS asset_prices;
