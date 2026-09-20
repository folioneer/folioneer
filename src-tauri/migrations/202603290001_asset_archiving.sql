-- Add is_archived flag for reversible soft-archive (distinct from is_deleted)
ALTER TABLE assets ADD COLUMN is_archived INTEGER NOT NULL DEFAULT 0;

-- Remove unique index on reference: spec R9 allows duplicate references (non-blocking warning only)
DROP INDEX IF EXISTS idx_assets_reference_active;
