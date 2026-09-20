-- Add last_sold_date and total_realized_pnl columns to holdings (ACD-043, ACD-045)
ALTER TABLE holdings ADD COLUMN last_sold_date TEXT;
ALTER TABLE holdings ADD COLUMN total_realized_pnl INTEGER NOT NULL DEFAULT 0;

-- Backfill total_realized_pnl from existing sell transactions
UPDATE holdings
SET total_realized_pnl = COALESCE(
    (SELECT SUM(t.realized_pnl)
     FROM transactions t
     WHERE t.account_id = holdings.account_id
       AND t.asset_id = holdings.asset_id
       AND t.transaction_type = 'Sell'
       AND t.realized_pnl IS NOT NULL),
    0
);

-- Backfill last_sold_date from latest sell transaction date
UPDATE holdings
SET last_sold_date = (
    SELECT MAX(t.date)
    FROM transactions t
    WHERE t.account_id = holdings.account_id
      AND t.asset_id = holdings.asset_id
      AND t.transaction_type = 'Sell'
);
