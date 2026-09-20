ALTER TABLE transactions ADD COLUMN realized_pnl INTEGER;
-- SQLite does not allow non-constant defaults in ALTER TABLE ADD COLUMN.
-- Existing rows get the epoch sentinel, which sorts before any real timestamp.
ALTER TABLE transactions ADD COLUMN created_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z';
