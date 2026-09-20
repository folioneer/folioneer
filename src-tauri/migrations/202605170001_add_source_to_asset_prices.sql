-- MKT-100 / MKT-101 — add provenance column to asset_prices
ALTER TABLE asset_prices ADD COLUMN source TEXT NOT NULL DEFAULT 'Manual';
UPDATE asset_prices SET source = 'Manual';
