# Git Hooks Setup

This directory contains git hooks that enforce code quality standards.

## Available Hooks

### commit-msg

- **Purpose:** Validate commit message format and policy compliance
- **Validates:**
  - Conventional commit format (`type: description`)
  - Valid types: `feat`, `fix`, `docs`, `test`, `chore`, `refactor`
  - Title under 72 characters
  - No co-author lines (not allowed per COMMIT_POLICY.md)
  - No test results in commit messages (those go in PRs)
  - Proper lowercase and no periods in title
- **Action:** Rejects commit if message violates COMMIT_POLICY.md

### pre-commit

- **Purpose:** Prevent commits that fail the fast checks for the scope they touch
- **Runs:** `npx prettier --check` on any staged Markdown, then `scripts/changed-scope.sh` on the staged files: frontend → `check.py --fast --frontend`; backend → `check.py --fast --backend`; both → `check.py --fast`; nothing a local check covers → nothing more
- **Action:** Rejects commit if any check for that scope fails

### pre-push

- **Purpose:** Catch what a commit slipped past, at the same cost model
- **Runs:** the same scoping over the commits being pushed (per ref, against the remote tip or the merge base with `main` for a new branch); delete-only pushes skip it
- **Action:** Rejects the push if a check for that scope fails. Tests, coverage, E2E and the build run in CI on the pull request, not here

## Setup Instructions

### One-time Setup

Configure git to use the hooks directory:

```bash
git config core.hooksPath .githooks
```

Or globally (for all repos):

```bash
git config --global core.hooksPath .githooks
```

### Verify Setup

```bash
# Check that hooks are configured
git config core.hooksPath
# Should output: .githooks
```

### Test the Hook

Try making a commit while a check fails (e.g., add a console.log). The commit will be rejected.

## Bypass Hooks (Not Recommended)

If you absolutely need to bypass hooks:

```bash
git commit --no-verify
```

## Hook Behavior

When you run `git commit`:

1. **commit-msg hook runs** → Validates message format and policy
   - Message invalid? → Commit rejected ❌
   - Message valid? → Continue to next hook

2. **pre-commit hook runs** → the fast checks for the staged scope (`scripts/changed-scope.sh`)
   - Any checks fail? → Commit rejected ❌
   - All checks pass? → Commit succeeds ✅

### If Rejected by commit-msg

- Fix the commit message (see error output)
- Re-run `git commit` with corrected message

### If Rejected by pre-commit

- Fix the issues (see error output)
- Run `python3 scripts/check.py` for the full check with details
- Re-run `git commit`

## Disabling Hooks Temporarily

```bash
# Run check script manually first
python3 scripts/check.py

# Then commit
git commit --no-verify
```

But this defeats the purpose! Hooks exist to maintain quality standards.
