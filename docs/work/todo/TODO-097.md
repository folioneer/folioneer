# TODO-097 — (fullstack) — A holding moves between two accounts at its cost

From gh275 (owner, 2026-10-17), which this entry closes when it ships. Moving an asset between accounts (an exchange to a hardware wallet, one broker to another) has no transaction kind: it takes a sale at cost, a withdrawal, a deposit and a purchase, which show trades that never happened, cash that never existed, and a distorted net cash input in both accounts.

**User value:** Moving holdings between accounts keeps their cost and history, the cash and net cash input of both accounts stay true, and transfers recorded the four-step way can be cleaned up.
**Done when:** A transfer moves quantity and cost from one account to another without cash and without a realised result, in the window and for an agent (`record_transfer`); a quantity received below the quantity sent is a fee in kind, leaving at its cost; between currencies the cost converts at the date's rate, recorded and shown; both sides are one operation, corrected and cancelled together, linked in both journals and absent from realised gains; four past recordings of a transfer can be replaced by one with the owner's approval, the result identical to recording the transfer from the start; tests hold both accounts' holdings, costs and cash before and after.
**Design:** none
**Open questions:**

- [x] Accounts in different currencies? — The cost converts at the date's rate, the rate recorded and shown. (owner, 2026-10-17)
- [x] In realised gains? — Ignored there; shown in both journals, linked. (owner, 2026-10-17)
