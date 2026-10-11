# FLOW-011 — Parts of the harness run only on the day they matter

- Kind: quality
- Observed: CI floated on the latest Rust until a new lint broke `main` (DEBT-059, fixed);
  the visual reference went stale when the month changed (DEBT-060, fixed); the release
  workflow's Windows steps run only at a release; the golden portfolio had never been
  read through the live account page (fixed in #87).
- Proposal: a weekly scheduled run of what otherwise runs only at a release — the
  Windows build without publishing.
- Costs: about 20 runner-minutes a week. Protects: a release day spent on a workflow
  bug.
- Since the 0.8.0 batch a pull request that touches the Rust code is compiled for Windows
  and runs the agent connection's tests there (Quality `windows`). What is left of this
  proposal is the release workflow's own steps: packaging and publishing.
