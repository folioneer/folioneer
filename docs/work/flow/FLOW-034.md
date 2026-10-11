# FLOW-034 — The generic part of the agentic setup is not all here yet

- Kind: quality
- Observed: `docs/agentic-setup.md` was ported on 2026-10-11 from the owner's other
  repository, whose Generic section is meant to read the same everywhere. Read side by
  side, six generic pieces have no counterpart here: the `/spec-writer` skill;
  `scripts/review-lanes.sh` (the lanes a change needs — here the rule is in
  `.github/workflows/review.yml` and in the `/run-entry` skill's prose);
  `scripts/scoped-checks.sh` (the hooks' fast checks — here written twice, in
  `pre-commit` and `pre-push`); `scripts/privacy-check.py`; `scripts/release-notes.py`. And `scripts/rule-homes.py` does
  not check the doc map here: a Markdown file outside every location of
  `docs/README.md` passes (`docs/advice-module-design.md` is in none).
- Proposal: take them one at a time, each as its own pull request, in this order: the
  doc-map check; `scoped-checks.sh`; `review-lanes.sh`; `/spec-writer`. For the release notes and the privacy
  check, the owner says which this repository needs: it holds no personal data of a
  third party.
- Costs: about half a day for the first four, each with its tests. Protects: one reading
  of the setup document for both repositories, and a skill or a script that can be
  carried from one to the other.
- Needs the owner: yes — which of the release notes and the privacy check are wanted here.
