-- IMP-051 — an added deposit: a deposit the import added before a purchase the file left
-- unfunded. The domain leaves it 0 on every other transaction.
ALTER TABLE transactions ADD COLUMN added_deposit INTEGER NOT NULL DEFAULT 0;
