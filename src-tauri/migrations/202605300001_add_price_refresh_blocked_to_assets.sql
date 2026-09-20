-- Per-asset price-refresh lock (MKT-150, ADR-014): when set, the asset is excluded
-- from every price-fetch task scope (MKT-151), preserving its most recently recorded
-- price. Independent of is_archived; existing assets default to not-locked.
ALTER TABLE assets ADD COLUMN price_refresh_blocked INTEGER NOT NULL DEFAULT 0;
