-- Add currency field to accounts (TRX-021, SEL-036)
-- Default 'EUR' applied to existing rows; app is not yet live so backfill is safe.
ALTER TABLE accounts ADD COLUMN currency TEXT NOT NULL DEFAULT 'EUR';
