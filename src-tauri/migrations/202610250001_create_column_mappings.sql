-- IMP-020 — a broker's column mapping, saved under a name and synced (IMP-023). The name is
-- unique case and accents ignored, checked when saved; a merge may leave two with one name,
-- so no constraint holds it here. The columns, the operation types and the asset matches
-- are JSON, read and written whole.
CREATE TABLE IF NOT EXISTS column_mappings (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    separator TEXT NOT NULL,
    decimal_mark TEXT NOT NULL,
    date_format TEXT NOT NULL,
    encoding TEXT NOT NULL,
    columns TEXT NOT NULL,
    operation_types TEXT NOT NULL,
    asset_matches TEXT NOT NULL,
    sync_logical_timestamp TEXT,
    sync_origin TEXT,
    sync_device_id TEXT
);
