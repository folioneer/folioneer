# TODO-063 — (backend) — An agent adds an asset the portfolio does not have

Raised on 2026-10-10 while comparing the three interfaces: the command line adds an asset (CLI-026), an agent cannot. Rebuilding an old account stops at the first instrument the owner no longer holds and never entered.

**User value:** An agent reading an old statement adds the instruments it names — a listed one by its ISIN, a custom one by its name — instead of stopping to ask the owner to create each.
**Done when:** a tool adds an asset of each kind through the rules of the window and of the command line (AST-031, AST-032), refusing the same asset twice; what an agent added is told to the owner in the window; the capabilities table says so (TODO-064).
**Design:** none
**Open questions:**

- [ ] An asset has no mark today. Should the assets table say which assets an agent added, as the journal does for transactions? (It adds a column or a badge: a design to validate.)
