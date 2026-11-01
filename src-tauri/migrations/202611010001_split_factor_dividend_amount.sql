-- SPL-010 / DIV-023 (#577) — a split's factor and a dividend's amount leave `quantity`, which
-- then only ever holds units or cash, for fields of their own.
ALTER TABLE transactions ADD COLUMN split_factor INTEGER;
ALTER TABLE transactions ADD COLUMN dividend_amount INTEGER;
UPDATE transactions SET split_factor = quantity, quantity = 0 WHERE transaction_type = 'Split';
UPDATE transactions SET dividend_amount = quantity, quantity = 0 WHERE transaction_type = 'Dividend';
