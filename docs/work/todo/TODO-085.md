# TODO-085 — (fullstack) — A price that is old says so, and why it was not updated

From gh242 (owner, 2026-10-12), which this entry closes when it ships: a listed asset was still valued at a price typed eighteen months earlier, about half its real value, and another's fetched price had stopped moving for ten weeks, with nothing drawing the eye to either.

Read on 2026-10-13: the window does say how old a price is — the holding row's "Current price" carries "Updated today" or "Updated Nd ago" and the price's source (MKT-140, MKT-142) — in the same quiet words whether it is one day or five hundred, and nothing counts the old ones on an account or on the portfolio. An agent is given the price's date and source by `list_holdings` and no judgement of them. Why a price was not fetched is said once, at the fetch, by the dialog of the prices left unupdated (MKT-172), and kept nowhere. One point of the issue falls away: a typed price is not kept against a fetch — the next fetch that succeeds writes today's price, which becomes the current one (ADR-012). A price stays old because no fetch succeeds for that asset, or because the build has no provider (the public one, MKT-210).

**User value:** The owner never reads a value or a gain built on a price nobody noticed was old, and knows which asset needs attention and why.
**Done when:** a holding whose price is older than the threshold is told apart from the others on its row, in both themes, and the account and the portfolio say how many of their holdings are in that case; an asset the last fetch left without a price says why, where its price shows; `list_holdings` and `portfolio_summary` give an agent the same judgement.
**Design:** none
**Open questions:**

- [ ] The threshold: one number of days for every asset, or one per asset class (a fund publishes less often than a share)?
- [ ] In the public build, which fetches nothing, every price is typed: is a typed price judged old by the same threshold, or never?
- [ ] The reason of a fetch that failed: kept until the next fetch that succeeds, on this computer only, or part of what sync carries?
