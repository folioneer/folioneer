-- FXR-024/025/100 — one dated rate observation per directed pair. `rate` is i64 micros
-- (units of to_currency per 1 from_currency, ADR-001); `source` is the text discriminant
-- Manual|Frankfurter|Ecb (FXR-100). Latest-write-wins upsert by the PK (ADR-012, FXR-025).
-- (from_currency, to_currency) is the leftmost PK prefix, so it already covers the FK to
-- currency_pairs — no separate FK index is needed (mirrors asset_prices.asset_id).
CREATE TABLE IF NOT EXISTS currency_rates (
    from_currency TEXT    NOT NULL,
    to_currency   TEXT    NOT NULL,
    date          TEXT    NOT NULL,
    rate          INTEGER NOT NULL,
    source        TEXT    NOT NULL,
    PRIMARY KEY (from_currency, to_currency, date),
    FOREIGN KEY (from_currency, to_currency)
        REFERENCES currency_pairs (from_currency, to_currency) ON DELETE CASCADE
);
