# TODO-076 — (backend) — An agent previews a purchase, a sale, an opening balance or a split before it is made

From gh207 (owner, 2026-10-11). A recording dated in the past changes the cash and the positions of every later date, and an agent cannot show the owner what it would do before doing it.

A read tool of its own, `preview_recording` — the name of a recording tool and its arguments — answers what that call would do, and does nothing: the transaction as it would be recorded, or the refusal the real call would give, and the account on the evening of the recording's date without it and with it. It is a read (AGT-013): an agent client asks no permission for it, and the recording tools stay as they are. Split on 2026-10-17 with the owner: this entry previews the four recordings the core already checks without writing — a purchase, a sale, an opening balance, a split (`validate_draft`, `validate_opening_balance_draft`, `validate_stock_split_draft`) — and brings the account computed in memory as of a date with a recording in, which no code computes today (`holding_snapshot_as_of` reads only what is stored); TODO-090 previews the rest.

**Size:** M — one pull request: the tool, the account as of a date with a recording in, a rule of the agent's spec.
**User value:** Claude shows the owner the exact result of a purchase, a sale, an opening balance or a split, or why it would be refused, before asking to go ahead.
**Done when:** `preview_recording` on `record_purchase`, `record_sale`, `record_opening_balance` and `record_split` changes nothing — a test reads the portfolio before and after; a preview gives the refusal the real call gives, a test per kind; the answer holds the account's holdings and cash on the evening of the recording's date, without the recording and with it; a preview counts as one of the session's reads, takes no place of the session's limit and is not announced; a tool it does not preview is refused, naming the ones it does.
**Design:** none
**Open questions:**

- [x] The holdings and cash "after it": as of today, or as of the recording's date — a past recording changes both? — As of the recording's date: the account on the evening of that date, with the recording in — the picture to set beside a statement of that date. The effect on today is read with `list_holdings` once it is recorded (owner, 2026-10-11).
- [x] A `dry_run` option on the recording tools, or a tool of its own? — A read tool of its own, `preview_recording` (a recording tool's name and its arguments): the client asks no permission for a preview, and every tool stays either a read or a write (owner, 2026-10-17).
- [x] Which calls are previewed? — The recordings, `correct_recording` and `record_price`; not `add_asset` nor `cancel_recording` (owner, 2026-10-17). This entry takes four recordings; TODO-090 the rest.
- [x] How a preview counts in the session, and whether the window says it? — One of the session's reads, no place of the limit, not announced (owner, 2026-10-17).
- [x] The answer: the account with the recording in only, or also without it? — Both, at the recording's date: what moves is what Claude shows the owner (owner, 2026-10-17).
