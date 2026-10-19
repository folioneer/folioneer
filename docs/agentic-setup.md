# The agentic setup

What each piece of the setup is for, in one line, and what it must never do. The
content of a piece differs from one repository to another; its responsibility does not.
This document is complete on its own: no command reads a second repository.

`scripts/rule-homes.py` fails when a script, a skill, an agent prompt or a git hook
exists without a line here, or when a line names a path that is gone. `/flow-audit`
reads each generic piece against its line. A piece is a list item opened by its path.

## Generic

Written to read the same in every repository that works this way.

### Documents

- `CLAUDE.md` — the index the agent reads every turn: authority, forbidden actions,
  pointers. Never: states a rule that has a home elsewhere.
- `docs/README.md` — the map: one location per kind of document. Never: holds a rule.
- `docs/workflow.md` — how work moves from an issue to the main branch, and who decides
  what. Never: says how code is written.
- `docs/commit-rules.md` — the commit format and what each type means. Never: restated
  in a hook's message beyond the line that failed.
- `docs/ubiquitous-language.md` — what the domain words mean, validated by the owner.
  Never: extended by the agent without a question.
- `docs/lessons.md` — failures worth teaching, one per entry. Never: a rule's home.
- `ARCHITECTURE.md` — where code lives. Never: why a choice was made (an ADR does).
- `.claude/rules/` — loads the rules document of a layer, or a document folder's
  `README.md`, when one of its files is read. Never: states the rule itself.
- `.claude/settings.json` — what the agent may run without asking, and what it may
  never run. Never: allows what `CLAUDE.md` forbids.
- `justfile` — the one entry point to every command. Never: holds logic a script
  should own and test.
- `coverage-gates.json` — the coverage floors. Never: lowered.
- `arch-allowlist.json` — the frozen architecture debt. Never: raised.

### Skills

- `.claude/skills/plan-batch/` — refines the candidates, proposes the next batch, and
  sets it on the Project once approved. Never: queues what the owner has not approved, nor commits.
- `.claude/skills/run-batch/` — runs a batch from its Queued items to its audit by
  chaining the other skills, and owns the batch's task list. Never: decides, edits the batch or releases.
- `.claude/skills/run-entry/` — runs one task through the ceremony (`docs/workflow.md` § 3),
  its task list only when run alone, and files what it met and did not fix before the issue
  closes. Never: a second task, a guess where a question is owed, nor a finding left in chat.
- `.claude/skills/design-proposal/` — renders the target state before anything a user
  sees changes, publishing the mocks in `folioneer/folioneer-design`. Never: builds what
  it shows, nor commits a mock to this repository.
- `.claude/skills/visual-proof/` — captures every state of a changed component, light
  and dark. Never: stands in for the design proposal.
- `.claude/skills/dep-audit/` — checks the dependencies for outdated versions and known
  vulnerabilities at a release's hand-over. Never: upgrades by itself.
- `.claude/skills/flow-audit/` — judges an ended batch from its records, forked with a
  fresh context, and posts it on the batch's issue. Never: takes the session's account,
  nor runs during a batch.

### Agent prompts

- `.claude/agents/review-protocol.md` — the steps, modes and report format every
  reviewer lane shares. Never: a lane's own checks.
- `.claude/agents/reviewer-infra.md` — reviews workflows, scripts, hooks, recipes and
  configuration. Never: application code.
- `.claude/agents/reviewer-guides.md` — reviews the documents and prompts the agent
  works from: contradictions, restated rules, length. Never: tooling or code.
- `.claude/agents/reviewer-security.md` — reviews the surface an attacker reaches: a
  change, and the whole command surface before a batch ends. Never: code quality.
- `.claude/agents/spec-reviewer.md` — judges a spec before anything is built from it.
  Never: checks code against it.
- `.claude/agents/spec-checker.md` — checks that every rule of a spec is implemented
  and tested. Never: judges the spec itself.
- `.claude/agents/contract-reviewer.md` — checks a contract against its spec and
  `docs/contracts/README.md`. Never: amends it.
- `.claude/agents/adr-reviewer.md` — checks an ADR's structure, gate and status.
  Never: writes one.

### Scripts

- `scripts/run-entry.sh` — runs one Queued item with nobody to ask. Never: a second
  task, nor past its time budget.
- `scripts/branch.sh` — where the branch left the main branch, and its diff, log and
  files. Never: changes the tree.
- `scripts/changed-scope.sh` — classifies changed paths into the checks they need.
  Never: runs a check.
- `scripts/check.py` — the quality checks of each layer, fast or full. Never: fixes
  what it reports unless asked to format.
- `scripts/e2e.sh` — the E2E suite under xvfb, or the specs given, for CI and `just e2e`.
  Never: runs in the harness.
- `scripts/harness.sh` — the merge gate of CI, locally, scoped to what moved. Never: a
  gate CI does not have.
- `scripts/coverage-gate.py` — fails when coverage of the code that carries logic
  drops under its floor. Never: lowers a floor.
- `scripts/arch-check.py` — enforces the architecture rules a machine can check.
  Never: lets the allowlist grow.
- `scripts/rule-homes.py` — one home per rule, one line here per piece. Never: judges
  what a document says.
- `scripts/review-lanes.sh` — only the reviewer lanes a change needs. Never: launches one.
- `scripts/review-path.sh` — where a reviewer writes its report. Never: writes it.
- `scripts/review-stop-reason.py` — says in one line why a reviewer session stopped in
  CI. Never: fails the job it explains.
- `scripts/watch-pr.py` — waits for a pull request's checks and says how they ended.
  Never: merges or re-runs.
- `scripts/merge.py` — rebases a branch whose checks are all green, has GitHub merge
  its pull request and deletes it. Never: merges on a red or missing check, pushes to
  the target, nor pushes over a branch that moved on the remote.
- `scripts/commit-check.sh` — refuses a commit that breaks `docs/commit-rules.md`,
  its author included, on a pull request and on `main` since the last release. Never: exempts an
  author other than a bot.
- `scripts/release.py` — computes the version, writes the changelog, lands them through
  a pull request and tags what merged, for the owner. Never: run by the agent (its own:
  `just batch-end` tags, `just batch-ends` lists), nor pushes to `main`.
- `scripts/mcpb-bundle.py` — builds the Claude Desktop bundle the release attaches, from
  the release's product name, version and binary. Never: holds the program or data.
- `scripts/flow-audit.py` — the figures of a batch, as one document. Never: reads
  them.
- `scripts/usage_log.py` — writes one line per run of a tool, locally. Never: in CI,
  nor anything about the data handled.
- `scripts/usage-log.sh` — the same line for a shell script. Never: changes the
  script's exit code.

### Git hooks

- `.githooks/pre-commit` — refuses a commit on the main branch, then runs the fast
  checks of the staged files. Never: tests.
- `.githooks/commit-msg` — refuses a title outside the commit format or an attribution
  line. Never: rewrites the message.
- `.githooks/pre-push` — the fast checks of the pushed commits. Never: what CI owns.
- `.githooks/pre-merge-commit` — refuses a merge commit, so history stays linear.
  Never: blocks a fast-forward.

## This project

The pieces only this repository has: the reviewers of its stack, the tools of its build.

- `.claude/agents/reviewer-backend.md` — the quality of Rust code against
  `backend-rules.md`. Never: layering, which is `reviewer-arch`'s.
- `.claude/agents/reviewer-frontend.md` — the quality and UX of TypeScript and React
  code against `frontend-rules.md`. Never: layering.
- `.claude/agents/reviewer-arch.md` — bounded contexts and the direction of data
  across both layers. Never: style.
- `.claude/agents/reviewer-e2e.md` — the WebdriverIO tests against `e2e-rules.md`.
  Never: runs when no E2E file changed.
- `.claude/agents/reviewer-sql.md` — the migrations, alone. Never: a query in
  application code.
- `scripts/start-app.sh` — starts the application in development mode. Never: touches
  the installed application's data.
- `scripts/dev-seed.py` — seeds the development data folder with a detached copy of
  the installed application's database. Never: writes to the source.
- `scripts/build-cli-sidecar.mjs` — builds the console program the Windows installer
  ships beside the application. Never: run by hand.
- `scripts/coverage-strip-tests.py` — removes inline Rust test modules from a coverage
  report. Never: removes production lines.
- `scripts/coverage-preflight.sh` — refuses a coverage run this machine would stop for
  lack of memory. Never: refuses a run that holds the foreground.
- `scripts/visual-proof-capture.mjs` — the browser capture behind `/visual-proof`.
  Never: run by hand.
- `scripts/verdict.sh` — runs a command, keeps its output in a log and prints its
  verdict in one line. Never: reads the verdict from the output instead of the exit code.
- `scripts/contract-check.py` — reports where a contract and the generated bindings
  differ. Never: edits either.
- `scripts/batch-specs.sh` — lists the specs changed since the last batch end, for
  `spec-checker`. Never: checks one.
- `scripts/e2e-scope.py` — says whether a pull request must run the E2E suite. Never:
  skips it for anything the suite builds or runs.
- `scripts/licence-check.py` — refuses a shipped dependency under a licence outside
  `licence-allowlist.json`. Never: extends the allowlist.
- `scripts/sync-format-check.sh` — refuses a change to a published sync format
  snapshot. Never: blocks a new version's snapshot.
