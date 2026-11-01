-- TRX-024 (#578): a non-cash quantity moves from millionths to hundred-millionths, exactly.
-- The Cash Holding's balance, what moves on the cash line, a split's factor and a dividend's
-- amount stay in millionths. A quantity that would not fit refuses the whole update, which
-- then leaves the portfolio as it was.
-- IRREVERSIBLE: one-time rescale of stored quantities; the millionths are not restored.
CREATE TEMP TABLE IF NOT EXISTS quantity_fits_check (too_large INTEGER NOT NULL CHECK (too_large = 0));
INSERT INTO quantity_fits_check
SELECT COUNT(*) FROM holdings
WHERE asset_id NOT LIKE 'system-cash-%' AND ABS(quantity) > 92233720368547758;
INSERT INTO quantity_fits_check
SELECT COUNT(*) FROM transactions
WHERE asset_id NOT LIKE 'system-cash-%' AND ABS(quantity) > 92233720368547758;
INSERT INTO quantity_fits_check
SELECT COUNT(*) FROM changes
WHERE record_kind = 'Transaction'
  AND content IS NOT NULL
  AND json_extract(content, '$.asset_id') NOT LIKE 'system-cash-%'
  AND json_extract(content, '$.transaction_type') NOT IN ('Split', 'Dividend')
  AND ABS(json_extract(content, '$.quantity')) > 92233720368547758;
INSERT INTO quantity_fits_check
SELECT COUNT(*) FROM held_back_changes
WHERE json_extract(payload, '$.record_kind') = 'Transaction'
  AND json_extract(payload, '$.content') IS NOT NULL
  AND json_extract(json_extract(payload, '$.content'), '$.asset_id') NOT LIKE 'system-cash-%'
  AND json_extract(json_extract(payload, '$.content'), '$.transaction_type')
      NOT IN ('Split', 'Dividend')
  AND ABS(json_extract(json_extract(payload, '$.content'), '$.quantity')) > 92233720368547758;
DROP TABLE quantity_fits_check;

UPDATE holdings SET quantity = quantity * 100 WHERE asset_id NOT LIKE 'system-cash-%';
UPDATE transactions SET quantity = quantity * 100 WHERE asset_id NOT LIKE 'system-cash-%';

-- SYN-038 — what this device has logged or holds back is published and applied in the new
-- form. A change written before version 13 keeps a split's factor or a dividend's amount in
-- `quantity`, which is not a quantity.
UPDATE changes
SET content = json_set(content, '$.quantity', json_extract(content, '$.quantity') * 100)
WHERE record_kind = 'Transaction'
  AND content IS NOT NULL
  AND json_extract(content, '$.asset_id') NOT LIKE 'system-cash-%'
  AND json_extract(content, '$.transaction_type') NOT IN ('Split', 'Dividend');

UPDATE held_back_changes
SET payload = json_set(
    payload,
    '$.content',
    -- Concatenated, the content is plain text and stays a string inside the payload, as a held
    -- change is read back; a bare json_set would nest it as an object.
    json_set(
        json_extract(payload, '$.content'),
        '$.quantity',
        json_extract(json_extract(payload, '$.content'), '$.quantity') * 100
    ) || ''
)
WHERE json_extract(payload, '$.record_kind') = 'Transaction'
  AND json_extract(payload, '$.content') IS NOT NULL
  AND json_extract(json_extract(payload, '$.content'), '$.asset_id') NOT LIKE 'system-cash-%'
  AND json_extract(json_extract(payload, '$.content'), '$.transaction_type')
      NOT IN ('Split', 'Dividend');
