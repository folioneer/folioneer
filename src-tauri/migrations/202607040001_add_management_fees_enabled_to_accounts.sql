-- FEE-075 — account-level gate for the % management-fee mechanism.
-- Existing accounts keep the mechanism (backfill true via the column default);
-- new accounts are created with the flag off at the application layer.
ALTER TABLE accounts ADD COLUMN management_fees_enabled INTEGER NOT NULL DEFAULT 1;
