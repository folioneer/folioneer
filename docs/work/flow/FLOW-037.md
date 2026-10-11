# FLOW-037 — The opening reading missed what the entry puts on a screen and what an ADR says

- Kind: quality + speed
- Size: S — one pull request: the workflow, two skills and their tests.
- Observed: in the 0.9.0 batch three questions reached the owner in mid-batch, all three
  of the kind the opening block exists to ask (`docs/workflow.md` § 3): the consent
  dialog named three kinds of recording where TODO-059 added four; "an agent's price
  never replaces one" stood against ADR-012's unconditional upsert; the session dialog
  speaks of transactions only. The reading had gone through the tools and the recorder —
  the code the entry changes — and not through the sentences that describe that code on a
  screen, nor the ADRs of the records it writes. The second cost a rewrite of a pushed
  pull request and one CI round. In the same two pull requests the tests were written
  after the code and not seen red first, which § 3 asks for; and the closing block put
  several questions to the owner at once, where `CLAUDE.md` says one at a time, each with
  its context.
- Proposal: the opening reading gets two named steps, in `docs/workflow.md` § 3 and in
  `/run-queue`: for every entry that adds or changes what an interface can do, search the
  locales and the specs for the sentences that list what it can do; for every record the
  entry writes, read the ADRs that name it. And `/run-entry` step 3 asks the pull
  request's body to say, per Done-when clause, whether its test was seen red.
- Costs: minutes per entry at the opening; a line per clause in the body. Protects: a
  pull request rewritten after its push, and a consent sentence that says less than what
  it allows.
- Queued first by the owner on 2026-10-11, so that the batch that follows is the first one read that way.
