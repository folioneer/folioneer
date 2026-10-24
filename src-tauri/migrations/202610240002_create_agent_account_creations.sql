-- AGT-068 — which accounts an agent created, and in which session. The mark lives beside
-- the account, is kept on this computer only and goes with it: deleting the account
-- removes its mark.
CREATE TABLE IF NOT EXISTS agent_account_creations (
    account_id TEXT PRIMARY KEY NOT NULL,
    agent TEXT NOT NULL,
    session TEXT NOT NULL,
    session_started_at TEXT NOT NULL,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);
