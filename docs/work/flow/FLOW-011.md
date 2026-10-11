# FLOW-011 — Parts of the harness run only on the day they matter

- Kind: quality
- Size: S — one pull request: a recipe and the hand-over's two rule lines.
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
- 2026-10-11: the release refused to start on its own validation — `cargo sqlx prepare
--check` asked for a database address, which sqlx-cli 0.9.0 on the owner's machine
  demands and 0.8.6 in CI does not (`docs/lessons.md`, 2026-05-22). Only a release runs
  that check locally, so nothing had shown it. `scripts/check.py` now gives the address;
  the tool's version on the machine is still not the one CI pins.
- 0.9.0, the same day: the release then stopped a second time, on a test that fails at
  random (two tests sharing a temporary folder), and a binary on disk older than the code
  listed 12 tools of 13. Three things in one day that the daily harness and CI passed and
  that only the release, or a run of the built program, showed.
- Decided (owner, 2026-10-11): the release's full validation runs at every hand-over of a
  batch, before `main` is called ready. A recipe runs exactly what `just release`
  validates — every test, the build, the full query check — and the hand-over of
  `docs/workflow.md` § 3 and `/run-queue` step 4 run it; about four minutes a batch on
  this machine. The weekly run in CI of the release workflow's own steps, the first
  proposal above, is not taken for now.
