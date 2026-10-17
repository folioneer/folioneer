# TODO-103 — (release) — A beta channel for early updates

From gh297 (owner, 2026-10-17), which this entry closes when it ships. A regression shipped to everyone at once is the worst incident with strangers' data. The update spec foresees channels and names a future beta (UPD-030).

**User value:** Volunteers get new features early; everyone else gets releases that have already been tried.
**Done when:** A user opts in and out of beta in the settings, and a beta build names its channel beside the version (UPD-031); a release goes to beta first and is promoted to everyone after 7 days, a serious report (data lost or damaged, wrong figures) blocking the promotion.
**Design:** none
**Open questions:**

- [x] How long in beta, and what blocks? — 7 days; a serious report blocks. (owner, 2026-10-17)
