-- ACC-026 — bank brand name metadata on accounts (free text, empty = unset).
ALTER TABLE accounts ADD COLUMN bank_name TEXT NOT NULL DEFAULT '';
