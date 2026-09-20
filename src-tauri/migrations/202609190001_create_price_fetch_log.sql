-- Price fetch log (MKT-201) — owned by use_cases/shared/price_fetch_log.rs.
-- Device-local singleton, outside the synced data: the local wall-clock moment
-- ("YYYY-MM-DDTHH:MM:SS") this installation last completed a price fetch, in the app or
-- by a scheduled download. A singleton has no identity to generate, so the key is an
-- INTEGER pinned to 1 by the CHECK, as in scheduled_fetch_configuration; no row means it
-- never fetched.
CREATE TABLE IF NOT EXISTS price_fetch_log (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_completed_at TEXT NOT NULL
);

-- A device that ran scheduled downloads before this table existed starts from its last
-- successful one.
INSERT OR IGNORE INTO price_fetch_log (id, last_completed_at)
SELECT 1, MAX(executed_at) FROM scheduled_fetch_runs
WHERE outcome = 'Succeeded'
HAVING MAX(executed_at) IS NOT NULL;
