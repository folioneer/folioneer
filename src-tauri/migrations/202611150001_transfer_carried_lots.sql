-- CBM-024 — the lots a transfer's side moved, as JSON: none on every other transaction, from
-- an account under the weighted average, and on a transfer recorded before its sides carried lots.
ALTER TABLE transactions ADD COLUMN carried_lots TEXT;
