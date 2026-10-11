# TODO-076 — (backend) — An agent previews a recording before it is made

From gh207 (owner, 2026-10-11). A recording dated in the past changes the cash and the positions of every later date, and an agent cannot show the owner what it would do before doing it.

An option on every recording tool, `dry_run`: every check of the real recording runs, nothing is written, nothing is taken of the session's limit. The answer is the transaction as it would be recorded, or the refusal it would get, and the account's holdings and cash after it. Part of it exists: the core checks a purchase, a sale, an opening balance and a split without writing (`validate_draft`, `validate_opening_balance_draft`, `validate_stock_split_draft`); it has no such check for a deposit, a withdrawal, a dividend or a price, and none answers the holdings and the cash that would result.

**User value:** Claude shows the owner the exact result of a recording, or why it would be refused, before asking to go ahead.
**Done when:** each recording tool takes `dry_run` and with it changes nothing — a test reads the portfolio before and after; a preview gives the refusal the real call gives; the answer holds the resulting holdings and cash.
**Design:** none
**Open questions:**

- [ ] The holdings and cash "after it": as of today, or as of the recording's date — a past recording changes both?
