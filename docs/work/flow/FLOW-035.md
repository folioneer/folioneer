# FLOW-035 — No file records which scripts and recipes ran

- Kind: quality
- Observed: `/flow-audit` counts what the agent used from the session's transcript and
  memory notes. In the 0.7.1 audit (`docs/work/audits.md`) `harness`, `format`, `test`
  and `check` are "not counted", #156 to #159 are outside the session it read, and a
  headless run (`just run-entry`) leaves no transcript it reads. The owner's other
  repository holds a usage log for this: `scripts/usage_log.py` with
  `scripts/usage-log.sh` (FLOW-034).
- Proposal: carry the two scripts over under the same names and with the same contract —
  one line per run in `logs/usage.log` (time, tool, duration, result), local and never
  committed, nothing written in CI nor when `USAGE_LOG=off`, a failure to write never
  failing the tool. Each Python script starts it, each shell script sources it, a recipe
  that runs no script depends on a `_used` recipe. `scripts/flow-audit.py` counts the
  runs per tool from the file, `/flow-audit` gives those figures in "Used and not used"
  and empties the file once they are recorded. Each of the two scripts gets its line in
  `docs/agentic-setup.md`.
- Costs: about two hours, with tests; a few milliseconds per run. Protects: an audit
  whose "used" figures are counted, for every session and for headless runs.
- Done when: a run of a recipe or a script leaves its line, with a test that a run in CI
  or under `USAGE_LOG=off` leaves none and that a write error does not change the exit
  code; `scripts/flow-audit.py` returns the count per tool, with a test; the skill reads
  them and empties the file.
- Needs the owner: no — taken on 2026-10-11, to run first so that the next batch is the
  first one logged.
