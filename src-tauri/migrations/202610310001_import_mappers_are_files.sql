
-- IMP-020 — an import mapper is a JSON file in the user's folder, never a record.
-- IRREVERSIBLE: data intentionally discarded; no window ever offered to save a column
-- mapping, so this table holds no user's data.
DROP TABLE IF EXISTS column_mappings;

-- IMP-020C — the mapper each folder a file was imported from remembers: this computer's own,
-- never synced. Keyed by folder: one row per folder.
-- A mapper's id is its file's name, so no foreign key holds it.
CREATE TABLE IF NOT EXISTS import_mapper_choices (
    folder TEXT PRIMARY KEY NOT NULL,
    mapper_id TEXT NOT NULL
);
