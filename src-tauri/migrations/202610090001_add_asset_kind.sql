-- How an asset is identified and priced (AST-030): Listed, Crypto, Custom or Cash. Every
-- existing asset takes the kind its class and ISIN make it (AST-034) — the Cash class is
-- cash, the digital-asset class is crypto, an ISIN makes it listed, anything else is
-- custom — and nothing else of it changes. Derived the same way on every device, so the
-- fill records no change to publish.
-- IRREVERSIBLE: the kind is derived from the class and the ISIN; no earlier value exists
-- to restore.
ALTER TABLE assets ADD COLUMN kind TEXT NOT NULL DEFAULT 'Custom';

UPDATE assets
SET kind = CASE
    WHEN asset_class = 'Cash' THEN 'Cash'
    WHEN asset_class = 'DigitalAsset' THEN 'Crypto'
    WHEN isin IS NOT NULL AND TRIM(isin) <> '' THEN 'Listed'
    ELSE 'Custom'
END;
