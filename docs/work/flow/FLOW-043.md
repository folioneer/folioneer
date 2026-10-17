# FLOW-043 — The workflow scripts do not run from a cloud agent session

- Kind: quality
- Observed: from gh314 (owner, 2026-10-17), not urgent. A Claude Code cloud session clones the repository shallow, and its GitHub proxy answers the REST API only (GraphQL: HTTP 403). `just queue next-id` stops on the shallow history (`scripts/work-queue.py`); `just watch-pr` and `just merge` read pull requests through `gh` commands that use GraphQL (`scripts/watch-pr.py`, `scripts/merge.py`, whose merge now goes through `gh pr merge`). `just` and the hooks path are set by hand.
- Proposal: read pull requests and check runs through the REST API only (`gh api repos/{owner}/{repo}/pulls`, `.../commits/{sha}/check-runs`), merge through `PUT .../pulls/{n}/merge`; `work-queue.py` deepens a shallow clone (`git fetch --unshallow`) or says what to run; the setup notes say what a cloud session needs (`just`, `git config core.hooksPath .githooks`). Done when the three recipes work where only REST is reachable and the clone is shallow, a script test covering each.
- Costs: about half a day. Protects: an agent in a cloud session follows the same workflow, merge guard included, as on the owner's machine.
- Needs the owner: no.
