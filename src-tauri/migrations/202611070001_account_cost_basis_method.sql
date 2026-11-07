-- CBM-010 / CBM-012 — each account's cost basis method, by its name: an account that existed
-- before is on the weighted average, and none of its figures moves. Synced with the account
-- (its written form is data format version 16).
ALTER TABLE accounts ADD COLUMN cost_basis_method TEXT NOT NULL DEFAULT 'WeightedAverage';
