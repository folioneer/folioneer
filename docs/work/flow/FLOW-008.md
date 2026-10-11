# FLOW-008 — The same reviewers run locally and in CI

- Kind: quality + speed
- Observed: each lane costs about a minute locally and about two in CI. They do not find
  the same things. In 0.6.0 the architecture lane was launched locally 4 times for 24
  backend and frontend launches, and CI's architecture lane then refused #124 (a
  conversion in the command layer): one round. The same skip cost a round on #77 in
  0.5.0.
- Verdict: **keep both.** The local run is what makes the first CI round green; CI is
  the one that cannot be skipped. The rule — the architecture lane goes with the backend
  or frontend lane every time — was not followed twice; the `/next-todo` skill now names
  the lane in its reviewer step. In 0.7.0 it was launched 9 times for 13 backend and
  frontend launches and CI's architecture lane refused nothing the local one had passed.
  In 0.7.1 it went with each of the 2 frontend launches; no backend code moved.
