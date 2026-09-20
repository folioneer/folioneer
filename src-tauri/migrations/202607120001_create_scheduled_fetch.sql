-- Scheduled daily price download (SPF) — tables owned by use_cases/scheduled_fetch/.
-- scheduled_fetch_configuration: device-wide singleton (SPF-011); the CHECK pins the
-- single row, seeded here so reads never face an empty table. trigger_time is a local
-- wall-clock "HH:MM" (SPF-014), defaulting to 22:15 (SPF-018).
CREATE TABLE IF NOT EXISTS scheduled_fetch_configuration (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 0,
    trigger_time TEXT NOT NULL DEFAULT '22:15'
        CHECK (trigger_time GLOB '[0-2][0-9]:[0-5][0-9]')
);

INSERT OR IGNORE INTO scheduled_fetch_configuration (id, enabled, trigger_time)
VALUES (1, 0, '22:15');

-- scheduled_fetch_runs: one row per execution — successful, failed, or guard-skipped
-- (SPF-050). outcome: 'Succeeded' | 'Failed' | 'SkippedAlreadyRun'.
-- trigger_date is the calendar day the run settles (latest pending trigger, SPF-021).
CREATE TABLE IF NOT EXISTS scheduled_fetch_runs (
    id TEXT PRIMARY KEY,
    executed_at TEXT NOT NULL,
    trigger_date TEXT NOT NULL,
    outcome TEXT NOT NULL,
    updated_count INTEGER NOT NULL,
    skipped_count INTEGER NOT NULL
);

-- The once-per-day guard (SPF-021) looks up successful runs by trigger_date.
CREATE INDEX IF NOT EXISTS idx_scheduled_fetch_runs_trigger_date
    ON scheduled_fetch_runs (trigger_date);
