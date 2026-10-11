# FLOW-010 — The local machine cannot carry a mutation sweep

- Kind: speed
- Observed: a local sweep with the commit hooks and another project's builds took the
  load to 53; the pre-push lint timed out three times at 600 s, and stopping it by
  process name killed the other project's builds.
- Proposal: sweeps run in CI only (`mutants.yml`); a background job is stopped by its
  task or process id, never by name. One line each in `docs/workflow.md`.
- Costs: nothing. Protects: an hour, and a neighbour's build.
- 0.6.0: the sweep ran in CI (about four hours, no local load). The two lines are still
  to write in `docs/workflow.md`.
- 0.7.0: no sweep. The same machine carries the backend coverage run only in the
  foreground (405 s, two build jobs); the owner asked on 2026-10-10 whether another
  computer could do the work.
- 0.9.0: no sweep. The two lines are still to write in `docs/workflow.md`.
