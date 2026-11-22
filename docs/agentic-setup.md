# The agentic setup

What each piece of the setup is for, in one line, and what it must never do. The
content of a piece differs from one repository to another; its responsibility does not.
For the pieces this project adds, this document is complete on its own.

`scripts/rule-homes.py` fails when a script, a skill, an agent prompt or a git hook
exists without a line here, or when a line names a path that is gone. `/flow:flow-audit`
reads each generic piece against its line. A piece is a list item opened by its path.

The skills, the reviewer prompts and the scripts of the method and the loop are daneel's
plugins (`CLAUDE.md`); this document lists what the project adds.

## Generic

Written to read the same in every repository that works this way.

### Documents

- `CLAUDE.md` — the index the agent reads every turn: authority, forbidden actions,
  pointers. Never: states a rule that has a home elsewhere.
- `docs/README.md` — the map: one location per kind of document. Never: holds a rule.
- `docs/workflow.md` — folioneer's additions to daneel's method and loop: its Project,
  its harness, its release steps, the infra lane's own checks. Never: says how code is
  written, nor restates the plugins.
- `docs/commit-rules.md` — the commit format and what each type means. Never: restated
  in a hook's message beyond the line that failed.
- `docs/ubiquitous-language.md` — what the domain words mean, validated by the owner.
  Never: extended by the agent without a question.
- `ARCHITECTURE.md` — where code lives. Never: why a choice was made (an ADR does).
- `.claude/rules/` — loads the rules document of a layer, or a document folder's
  `README.md`, when one of its files is read. Never: states the rule itself; never an `@`
  import, which loads at launch whatever the rule's `paths:` scope.
- `.claude/settings.json` — what the agent may run without asking, what it may
  never run, and the plugins enabled at their pin. Never: allows what `CLAUDE.md` forbids,
  nor edited by the agent.
- `.claude/daneel.json` — the project's part of the plugins: its Project, its rules
  documents per reviewer lane, the paths it adds to a lane, its recipes and release
  files. Never: a key no plugin piece reads.
- `justfile` — the one entry point to every command. Never: holds logic a script
  should own and test.
- `coverage-gates.json` — the coverage floors. Never: lowered.
- `arch-allowlist.json` — the frozen architecture debt. Never: raised.

### Scripts

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
- `scripts/review-stop-reason.py` — says in one line why a reviewer session stopped in
  CI. Never: fails the job it explains.
- `scripts/commit-check.sh` — refuses a commit that breaks `docs/commit-rules.md`,
  its author included, on a pull request and on `main` since the last release. Never: exempts an
  author other than a bot.
- `scripts/mcpb-bundle.py` — builds the Claude Desktop bundle the release attaches, from
  the release's product name, version and binary. Never: holds the program or data.
- `scripts/usage_log.py` — writes one line per run of a tool, locally. Never: in CI,
  nor anything about the data handled.
- `scripts/usage-log.sh` — the same line for a shell script. Never: changes the
  script's exit code.

### Git hooks

- `.githooks/pre-commit` — refuses a commit on the main branch, then runs the fast
  checks of the staged files. Never: tests.
- `.githooks/commit-msg` — refuses a title outside the commit format (its description's
  length and its scope included) or an attribution line. Never: rewrites the message.
- `.githooks/pre-push` — the fast checks of the pushed commits. Never: what CI owns.
- `.githooks/pre-merge-commit` — refuses a merge commit, so history stays linear.
  Never: blocks a fast-forward.

## This project

The pieces only this repository has: the tools of its build.

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
- `scripts/verdict.sh` — runs a command, keeps its output in a log and prints its
  verdict in one line. Never: reads the verdict from the output instead of the exit code.
- `scripts/contract-check.py` — reports where a contract and the generated bindings
  differ. Never: edits either.
- `scripts/e2e-scope.py` — says whether a pull request must run the E2E suite. Never:
  skips it for anything the suite builds or runs.
- `scripts/licence-check.py` — refuses a shipped dependency under a licence outside
  `licence-allowlist.json`. Never: extends the allowlist.
- `scripts/sync-format-check.sh` — refuses a change to a published sync format
  snapshot. Never: blocks a new version's snapshot.
