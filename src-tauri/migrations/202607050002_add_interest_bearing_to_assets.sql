-- Per-asset interest-bearing opt-in flag (AST-024, INT-012): when set, the asset
-- is an eligible target for Interest credits alongside the always-eligible cash
-- line. Existing assets default to not-interest-bearing.
ALTER TABLE assets ADD COLUMN interest_bearing INTEGER NOT NULL DEFAULT 0;
