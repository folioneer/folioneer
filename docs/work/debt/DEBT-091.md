# 2026-10-10 — DEBT-091 — Three commands granted to the CI reviewers can write or run a program

- Found by: `reviewer-security`, before the 0.7.0 release
- Where: `.github/workflows/review.yml` — `--allowedTools`: `Bash(sed -n *)`, `Bash(sort *)`, `Bash(git diff *)`
- Severity: 🟡
- Observation: the reviewers are granted commands by name so that none can write or run code on the runner (DEBT-082's fix). Three of the grants can: GNU `sed` runs a shell command or writes a file from its own script (`e`, `w`), `sort` runs a program (`--compress-program`) and writes (`-o`), `git diff` writes (`--output=`) and reads any path (`--no-index`). A pull request whose text steers its reviewer could use them with the job's environment. Not changed the day of a release: a reviewer that loses a command it leans on spends its turns on refusals (DEBT-087)
- User value: None directly — a pull request cannot make its own reviewer run code.
- Done when: `sed -n` is not granted (the file reader takes an offset and a limit); `sort` and `git diff` are not granted, or are granted in forms that take no output or program option; a test of the workflow holds the list; a wide pull request still gets its reports within the turn limit.
