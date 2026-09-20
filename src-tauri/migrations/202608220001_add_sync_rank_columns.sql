-- Per-record rank of the change that produced the row's current state (CFR-014), on every
-- synced record kind (SYN-021): the logical timestamp (CFR-010, zero-padded 20-char decimal so
-- lexicographic order equals numeric order), the origin ('User' | 'Application', CFR-016) and the
-- device that made the change. NULL on all three is the "never ranked" sentinel and compares
-- below every real rank; rows are stamped by the first publish (SYN-013), never by SQL backfill.
-- `holdings` is deliberately excluded (derived, SYN-022); `scheduled_fetch_*` too (device-local,
-- SYN-023). Grouped by owning bounded context so each column set matches the repository that
-- stamps it.

-- account BC
ALTER TABLE accounts ADD COLUMN sync_logical_timestamp TEXT;
ALTER TABLE accounts ADD COLUMN sync_origin TEXT;
ALTER TABLE accounts ADD COLUMN sync_device_id TEXT;

ALTER TABLE transactions ADD COLUMN sync_logical_timestamp TEXT;
ALTER TABLE transactions ADD COLUMN sync_origin TEXT;
ALTER TABLE transactions ADD COLUMN sync_device_id TEXT;

ALTER TABLE fee_schedules ADD COLUMN sync_logical_timestamp TEXT;
ALTER TABLE fee_schedules ADD COLUMN sync_origin TEXT;
ALTER TABLE fee_schedules ADD COLUMN sync_device_id TEXT;

ALTER TABLE holding_notes ADD COLUMN sync_logical_timestamp TEXT;
ALTER TABLE holding_notes ADD COLUMN sync_origin TEXT;
ALTER TABLE holding_notes ADD COLUMN sync_device_id TEXT;

-- asset BC
ALTER TABLE assets ADD COLUMN sync_logical_timestamp TEXT;
ALTER TABLE assets ADD COLUMN sync_origin TEXT;
ALTER TABLE assets ADD COLUMN sync_device_id TEXT;

ALTER TABLE categories ADD COLUMN sync_logical_timestamp TEXT;
ALTER TABLE categories ADD COLUMN sync_origin TEXT;
ALTER TABLE categories ADD COLUMN sync_device_id TEXT;

ALTER TABLE asset_prices ADD COLUMN sync_logical_timestamp TEXT;
ALTER TABLE asset_prices ADD COLUMN sync_origin TEXT;
ALTER TABLE asset_prices ADD COLUMN sync_device_id TEXT;

-- currency BC
ALTER TABLE currency_pairs ADD COLUMN sync_logical_timestamp TEXT;
ALTER TABLE currency_pairs ADD COLUMN sync_origin TEXT;
ALTER TABLE currency_pairs ADD COLUMN sync_device_id TEXT;

ALTER TABLE currency_rates ADD COLUMN sync_logical_timestamp TEXT;
ALTER TABLE currency_rates ADD COLUMN sync_origin TEXT;
ALTER TABLE currency_rates ADD COLUMN sync_device_id TEXT;
