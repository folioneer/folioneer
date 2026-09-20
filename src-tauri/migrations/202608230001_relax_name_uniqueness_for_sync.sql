-- CFR-035 — duplicate names coexist after a multi-device merge: two accounts, or two
-- categories, created or renamed independently on two devices may share a name once their
-- changes meet, each with its own history, until the user renames one. The uniqueness rules
-- ACC-003 / CAT-001 bind the name being set — enforced by the service guards on create and
-- rename — not names that already clash, so the schema must allow two rows with one name.
-- Both case-insensitive UNIQUE indexes become plain indexes with the same expressions, so
-- the name lookups stay fast.
DROP INDEX IF EXISTS idx_accounts_name_lower;
CREATE INDEX IF NOT EXISTS idx_accounts_name_lower ON accounts(LOWER(name));

DROP INDEX IF EXISTS idx_categories_active;
CREATE INDEX IF NOT EXISTS idx_categories_active
ON categories(LOWER(name))
WHERE is_deleted = 0;
