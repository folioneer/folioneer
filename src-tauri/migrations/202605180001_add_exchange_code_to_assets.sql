-- AST-021 / AST-022 — add optional canonical trading-venue identifier on Asset.
-- Stores the ISO 10383 MIC of the venue (e.g. "XPAR", "XNAS"); label is resolved
-- at read time from the canonical Exchange constant in domain/exchange.rs.
-- Existing assets get NULL; MKT-110 step 2 (lowercase reference fallback) preserves
-- the US-ticker happy path so no backfill is needed.
ALTER TABLE assets ADD COLUMN exchange_code TEXT NULL DEFAULT NULL;
