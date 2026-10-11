# FLOW-036 — A release nobody audits afterwards leaves no figures

- Kind: quality
- Size: S — one pull request: a check of the queue script with its test.
- Observed: 0.8.0 was released at the end of 2026-10-10 and no session followed it:
  `/prune` and `/flow-audit`, which `docs/workflow.md` § 3 asks for after a release, never
  ran. The 0.9.0 audit shows the 0.8.0 batch through the script only; every figure a
  session counts — the questions put to the owner, the defects the reviewers found, what
  was used — is "not counted" for it, and the comparison beside it is half empty.
- Proposal: `/plan-queue` says, in its "In flight" section, when the last published
  release is later than the last measured section of `docs/work/audits.md`, so that the
  next session starts by running the two audits it owes. One check in
  `scripts/work-queue.py`, with its test.
- Costs: about an hour. Protects: the audit's "was" column, and a prune that would
  otherwise skip a release's files.
- Queued by the owner on 2026-10-11, to be amended if needed when it is built.
