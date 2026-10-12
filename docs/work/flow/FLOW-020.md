# FLOW-020 — Nobody counts why a pull request goes round again

- Kind: speed
- Observed: 24 rounds of 61 came after the first in 0.6.0, the same share as in 0.5.0,
  and the cause — a rebase after another merge, a fix after a review, a failed check —
  was counted by hand once and not at all this time. FLOW-003's verdict rests on it.
- Proposal: `scripts/flow-audit.py` sorts each later round by comparing its commit with
  the round before: same changed files (a rebase) or different ones (a fix), and whether
  the round before had a failed check.
- Costs: about two hours, with tests; one `gh` call per later round at audit time.
  Protects: the decision on a merge queue being taken on a figure.
- 0.7.0: not queued, and the causes were not counted a second time.
- 0.7.1: one later round in eighteen. While the entries are stacked there is little to
  sort; the figure argues for waiting until the share rises again.
- 0.9.0: 4 later rounds in 22; one cause counted by hand, three not. The share stays
  low while entries are stacked; the figure still argues for waiting.
- 0.10.0 batch (2026-10-12): no round came after the first, of 22: nothing to count. The batch before counted 1 cause of 4.
