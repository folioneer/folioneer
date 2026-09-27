-- When this device's last successful sync ended (SYN-063): written only by a run that
-- finished without a failure, so the header never presents a failed attempt as a sync,
-- and kept across restarts. Device-local, never published. NULL until the first success.
ALTER TABLE sync_device ADD COLUMN last_synced_at TEXT;
