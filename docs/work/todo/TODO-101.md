# TODO-101 — (backend) — An agent records several transactions as one batch, all or nothing

From gh208 (owner, 2026-10-17), which this entry closes when it ships. A year-end statement is about ten recordings; when the fifth is refused the first four stay and the account is half rebuilt. TODO-070 holds the other half of the issue — cancelling a batch as one; this entry is the all-or-nothing recording.

**User value:** An agent records a year of history in one step, and either all of it lands or none of it does.
**Done when:** `record_batch` takes a list of recordings, each with its kind and the arguments of its tool, checks and records them in one database transaction, and answers the ids and one batch id; one refusal refuses the whole batch, names the line and gives its code, and leaves the account untouched (tested); a batch counts against the session's limit as many recordings as it holds; `preview_recording` previews a batch as it would land.
**Design:** none
**Open questions:** none
