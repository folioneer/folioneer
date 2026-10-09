-- AGT-045 — which transactions an agent recorded, and in which session. The mark lives
-- beside the transaction and goes with it: removing the transaction removes its mark.
CREATE TABLE IF NOT EXISTS agent_recordings (
    transaction_id TEXT PRIMARY KEY NOT NULL,
    agent TEXT NOT NULL,
    session TEXT NOT NULL,
    session_started_at TEXT NOT NULL,
    FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE CASCADE
);
