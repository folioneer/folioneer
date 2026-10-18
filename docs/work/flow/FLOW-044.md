# FLOW-044 — The dependency audit before a release waits on the owner

- Kind: speed + quality
- Observed: in the 0.12.0 batch the hand-over's `/dep-audit` ran on 2026-10-17 before three more pull requests merged (#309, #320, #322); the owner then had to invoke `/dep-audit` themselves before `just release`, as CLAUDE.md's release line and `docs/workflow.md` § 3 assign it, and said the agent should run it itself (owner, 2026-10-17).
- Proposal: the agent runs `/dep-audit` on `main` as it will be released — once the last pull request before the release has merged and the runs of `main`'s head are green — and hands the release over with its verdict ("🔴 0: `just release -y`", or the blocker). CLAUDE.md's release line and `docs/workflow.md` § 3 (the hand-over, the release) say so; the owner's part of a release is `just release -y` and publishing the draft. `/run-queue`'s Step 4 runs the audit last, after any pull request the hand-over itself opened.
- Costs: one audit of a few minutes more per release when `main` moved after the hand-over; three lines of rules. Protects: a release never ships a dependency `main` gained after its audit, and the owner is not asked for a step the agent can run.
- Needs the owner: no — asked for in the batch.
- Size: S — one pull request: three lines of rules in CLAUDE.md, `docs/workflow.md` § 3 and `/run-queue`.
- Queued by the owner on 2026-10-18.
