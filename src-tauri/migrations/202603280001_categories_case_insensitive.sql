-- Replace case-sensitive category name index with case-insensitive equivalent
DROP INDEX IF EXISTS idx_categories_active;
CREATE UNIQUE INDEX IF NOT EXISTS idx_categories_active
ON categories(LOWER(name))
WHERE is_deleted = 0;
