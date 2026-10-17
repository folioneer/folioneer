# TODO-099 — (fullstack) — An account computes its gains by FIFO lots, when its country does

From gh298 (owner, 2026-10-17), which this entry closes when it ships. Realised gains use the weighted average cost (TRX-030), the rule in France. Germany, Spain and Poland use FIFO: a German user sees gains that match neither their broker nor their return. The year-end figures (TODO-100) need it.

**User value:** A user in Germany, Spain or Poland sees the realised gains their broker and tax office see.
**Done when:** Each account has a cost basis method, weighted average or FIFO, preselected from its wrapper's country (TODO-091) and changeable, a change recomputing its history; existing accounts stay on weighted average and the golden portfolio does not move; with FIFO every purchase, opening balance, free share and transfer in (TODO-097) is a lot that sales consume oldest first, tested on partial sales across lots, free shares, a split and a transfer; a holding's lots can be shown; an agent reads the method and the lots.
**Design:** none
**Open questions:**

- [x] Per account or per portfolio? — Per account, preselected from the wrapper's country, changeable. (owner, 2026-10-17)
- [x] Other methods? — Weighted average and FIFO only. (owner, 2026-10-17)
