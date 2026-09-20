-- AST-023 — add optional ISO 6166 ISIN identity on Asset, separate from `reference`.
-- Stores the normalized 12-character uppercase form (WEB-016); validation is enforced
-- domain-side before persistence so the column itself stays as plain TEXT NULL.
-- Existing assets get NULL; the field is informational (not consulted by MKT-110
-- symbol derivation, which keeps using `reference`).
ALTER TABLE assets ADD COLUMN isin TEXT NULL DEFAULT NULL;
