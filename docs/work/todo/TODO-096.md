# TODO-096 — (fullstack) — Backups, and an open export: the user's data stays theirs

From gh291 (owner, 2026-10-17), which this entry closes when it ships. Once strangers keep years of history in Folioneer, two failures cost the most trust: a migration that damages a portfolio, and a user who cannot leave with their data. Today there is no copy before a migration, no backup to restore from the window, and no documented export.

**User value:** A user can trust a new version not to lose their history, and can always take their data elsewhere.
**Done when:** Before every migration the database is copied aside, the last copies kept, and a failed migration restores the copy and says so (a test forces one); a backup is saved and restored from the window, with a warning before overwriting, and a restored portfolio equals the original (golden portfolio); accounts, assets, transactions and prices export as CSV and JSON in a documented format that the CSV import (TODO-025) re-imports to the same figures.
**Design:** none
**Open questions:**

- [x] How many automatic copies are kept, and where (next to the database, or in the sync folder)? — The last 5, next to the database on this computer, never in the sync folder (gh291). (owner, 2026-10-18)
- [x] Does a restore on one computer reach the others through sync, or stay local until confirmed? — It stays local until the user confirms it: the portfolio is marked restored and not yet shared, sync sends nothing, the window says so; confirming propagates it, cancelling returns to the state before (gh291). (owner, 2026-10-18)
