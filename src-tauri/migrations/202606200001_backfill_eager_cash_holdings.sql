-- CSH-012 — eager cash line backfill. Every account gains a 0-balance Cash Holding;
-- this one-off migration backfills accounts created under the old lazy-creation path.
-- Cross-context: seeds asset-context rows (categories, assets) and account-context rows
-- (holdings) — the same records AssetService::seed_cash_asset / the account aggregate
-- produce at runtime (mirror their column values exactly).
--
-- Insert-if-absent throughout: a Cash Holding created under the old lazy path (with its
-- balance) is preserved untouched. FK enforcement is deferred to commit so the migration
-- runs regardless of the connection's foreign_keys state; rows are still inserted
-- parent -> child (category -> assets -> holdings).
PRAGMA defer_foreign_keys = ON;

-- (1) System Cash category (CSH-017). Matches core::cash::SYSTEM_CASH_CATEGORY_{ID,LABEL}.
INSERT OR IGNORE INTO categories (id, name)
VALUES ('system-cash-category', 'generic.cash');

-- (2) One Cash Asset per distinct existing-account currency (CSH-010 / CSH-011).
-- Matches AssetService::seed_cash_asset: id system-cash-{lower(ccy)}, name "Cash {UPPER}",
-- reference {UPPER}, asset_class 'Cash', risk_level 1; other columns take their defaults.
INSERT OR IGNORE INTO assets (id, name, reference, asset_class, category_id, currency, risk_level)
SELECT DISTINCT
    'system-cash-' || lower(a.currency),
    'Cash ' || upper(a.currency),
    upper(a.currency),
    'Cash',
    'system-cash-category',
    a.currency,
    1
FROM accounts a;

-- (3) A 0-balance Cash Holding for every account that does not already have one
-- (CSH-012). average_price = 1_000_000 (1.0 micros, cash is its own unit, ADR-001).
INSERT INTO holdings (id, account_id, asset_id, quantity, average_price, total_realized_pnl, last_sold_date)
SELECT
    'cash-' || a.id,
    a.id,
    'system-cash-' || lower(a.currency),
    0,
    1000000,
    0,
    NULL
FROM accounts a
WHERE NOT EXISTS (
    SELECT 1 FROM holdings h
    WHERE h.account_id = a.id
      AND h.asset_id = 'system-cash-' || lower(a.currency)
);
