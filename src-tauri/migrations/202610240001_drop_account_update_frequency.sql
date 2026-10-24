-- ACC-004 (#405) — an account no longer has an update frequency: nothing read it but the
-- month view of performance, now offered for every account.
-- Safe plain DROP: SQLite 3.46 (libsqlite3-sys 0.30.1) and the column carries no index,
-- constraint or foreign key.
ALTER TABLE accounts DROP COLUMN update_frequency;
