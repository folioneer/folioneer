# TODO-090 — (backend) — An agent previews cash, a dividend, free shares, a price or a correction before it is made

Split from TODO-076 on 2026-10-17 with the owner, whose answers on that entry hold here too; after it. `preview_recording` (TODO-076) previews a purchase, a sale, an opening balance and a split; this entry adds the calls whose checks the core runs only while writing: a deposit, a withdrawal, a dividend, free shares — each needing its check without saving — a price, which answers the price it would replace at that date and moves no holding, and a correction, which answers the account before and after it.

**Size:** M — one pull request: the checks without saving of the remaining kinds, the correction and the price in the tool, the rule of the agent's spec.
**User value:** Claude shows the owner the exact result of any recording, correction or price, or why it would be refused, before asking to go ahead.
**Done when:** `preview_recording` on `record_deposit`, `record_withdrawal`, `record_dividend`, `record_free_shares`, `record_price` and `correct_recording` changes nothing — a test reads the portfolio before and after; a preview gives the refusal the real call gives, a test per kind; a recording or a correction answers the account on the evening of its date without it and with it; a price answers the price it would replace at that date, or that there is none.
**Design:** none
**Open questions:** none
