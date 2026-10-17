# TODO-026 — (fullstack) — Asset allocation: how the portfolio splits, and how far it drifts from targets

The application answers "how did it perform?" in several views and "what do I own, and is it balanced?" in none. Everything needed is already recorded on each asset — class, category, currency, risk level — and the portfolio is already valued in EUR across accounts (the Global Value); what is missing is the read that groups that value and the view that shows it.

Proposal: an allocation view over the whole portfolio, filterable to one account, that splits the current value by asset class, by category, by currency and by risk level — amount and share of the total for each slice, cash included, with a drill-down to the holdings behind a slice. The backend computes every figure through the same valuation path as the Global Value (never a second one), including how an asset without a price is counted. Second step, same entry if it stays small: a target share per asset class, stored with the portfolio (and therefore synced), and the drift of each class from its target — which also closes the target-weights question left open in TODO-011's design document. No advice, no "you should": shares, targets, drift.

**User value:** The user sees at a glance how their money is spread — by kind of asset, by currency, by risk — and how far that is from what they intended.
**Done when:** The view shows the four splits with amount and share, for the whole portfolio and for one account, in English and French; the totals equal the Global Value to the cent; a slice opens the holdings behind it; targets per asset class can be set and each class shows its drift; the figures are computed and tested in the backend, including an unpriced asset and a foreign-currency holding; screenshots in light and dark mode are committed.
**Design:** none
**Open questions:**

- [x] Where does it live — a new entry in the navigation, or a tab beside the accounts list and the global performance? (Recommended: its own navigation entry; it is a third question about the portfolio, not a detail of the other two.) — Its own navigation entry. (owner, 2026-10-17)
- [x] Targets in the first version, or splits only first? (Recommended: splits first if targets push the work past one PR per layer; the entry stays open for the second step.) — Splits first; targets and drift follow in this entry. (owner, 2026-10-17)
- [x] Targets per asset class only, or also per category and per currency? And is a drift just shown, or flagged past a tolerance (for instance the 5/25 rule mentioned in TODO-011)? (Recommended: per class, flagged past a tolerance you set once.) — Per asset class. The free category may disappear as unused, so it is not a target axis. (owner, 2026-10-17)
