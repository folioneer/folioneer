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
  In 0.9.0 it went with each of the 2 backend launches, and CI's architecture lane refused
  #200 all the same: the local run had graded the write-time check against ADR-012 as a
  warning with a decision for the owner, CI's as a critical. The same finding, two
  grades: one round, and the decision the owner then took.
- 0.10.0 batch (2026-10-12): 31 local reviewer launches, 7 real defects found before a push, and no Review failure in CI where the batch before had 2. The verdict stands.
- 0.11.0 batch (2026-10-16): 38 local reviewer launches, 9 real defects found before a push; CI's spec lane still found a 🔴 twice (#253, #260), both on text written in answer to the local review and not reviewed again (FLOW-041). The verdict stands.
- 0.12.0 batch (2026-10-17): the architecture lane was launched 15 times for 12 backend and 12 frontend launches, with each of them; CI's lanes refused one pull request on a false positive (#280) and failed two on their own verdict (#309, #311 — fixed in #311). The verdict stands.
