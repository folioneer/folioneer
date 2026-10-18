-- ACC-041 — an account's own country (ISO 3166-1 alpha-2) and the identifier of its tax
-- wrapper (ACC-040). Both optional: an account that existed before has neither, and nothing
-- else changes for it. Synced with the account (its written form is data format version 4).
ALTER TABLE accounts ADD COLUMN country TEXT;
ALTER TABLE accounts ADD COLUMN wrapper TEXT;
