-- FXR-013/014/054 — a CurrencyPair is a durable, directed (from -> to) reference record.
-- Natural composite PK; the two currencies always differ (enforced in the domain, FXR-011/023).
CREATE TABLE IF NOT EXISTS currency_pairs (
    from_currency TEXT NOT NULL,
    to_currency   TEXT NOT NULL,
    PRIMARY KEY (from_currency, to_currency)
);
