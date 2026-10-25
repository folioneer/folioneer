#!/usr/bin/env bash
# The commit rules of docs/commit-rules.md, on every commit of a range (`BASE..HEAD`):
# CI runs it on a pull request's commits and on what main holds since the last release.
# The commit-msg hook checks the same message rules before a commit exists; the author
# can only be read here.
# Not -e: every commit is checked and every failure told before the verdict.
set -uo pipefail

# The owner's commit identity: an agent commits as the owner, never as itself. GitHub
# writes the same address with the account's id in front (`<id>+phileggel@…`).
OWNER_EMAIL="phileggel@users.noreply.github.com"
REGEX='^(feat|fix|docs|test|chore|refactor|ci)(\([^)]+\))?!?: .+'
# The scopes a person may write: the list of docs/commit-rules.md § Scope.
SCOPES=$(sed -n '/<!-- scopes:start -->/,/<!-- scopes:end -->/p' \
  "$(dirname "${BASH_SOURCE[0]}")/../docs/commit-rules.md" | grep -oE '^\| *`[a-z-]+`' | tr -d '|` ')
if [ -z "$SCOPES" ]; then
  echo "::error::no scope list found in docs/commit-rules.md § Scope"
  exit 1
fi

RANGE="${1:?usage: commit-check.sh <base>..<head>}"
# --no-merges skips the synthetic merge commit GitHub creates at HEAD for
# pull_request events (it represents "what main looks like if this PR
# merged" — not a real PR commit); main's history is linear.
SHAS=$(git rev-list --no-merges "$RANGE") || exit 1

# A commit that breaks the rules and stays on main, as the owner accepted it: listed by
# its full sha, one per line, `#` for the why. Without it a bad commit keeps main red
# until a release, which refuses to start on red.
ACCEPTED="$(git rev-parse --show-toplevel)/commit-check-accepted.txt"

FAIL=0
for sha in $SHAS; do
  if [ -f "$ACCEPTED" ] && grep -qx "$sha" <(sed -E 's/[[:space:]]*(#.*)?\r?$//' "$ACCEPTED"); then
    echo "ℹ $sha — accepted as it is (commit-check-accepted.txt)"
    continue
  fi
  TITLE=$(git log -1 --pretty=%s "$sha")
  # A `fixup!` commit is folded by `just merge` into the commit it
  # names; the rules apply to the title it carries.
  TITLE=${TITLE#fixup! }
  BODY=$(git log -1 --pretty=%b "$sha")
  AUTHOR=$(git log -1 --pretty='%an <%ae>' "$sha")
  AUTHOR_EMAIL=$(git log -1 --pretty=%ae "$sha")

  # Conventional-commit format is load-bearing for release.py
  # changelog parsing — enforced for every author, including bots.
  if ! echo "$TITLE" | grep -E "$REGEX" >/dev/null; then
    echo "::error::$sha — invalid conventional format: $TITLE"
    FAIL=1
  fi

  # The remaining checks (author, description length, scope, trailing
  # period, body length, no Co-Authored-By) are human-writing-hygiene rules.
  # Bot commits (Dependabot, Renovate) carry auto-generated
  # titles/bodies that legitimately bypass them — e.g. "across
  # N directories with M updates" titles and multi-page release
  # notes. Enforce only for human authors.
  if [[ "$AUTHOR_EMAIL" != *"[bot]@"* ]]; then
    if [ "$AUTHOR_EMAIL" != "$OWNER_EMAIL" ] && [[ "$AUTHOR_EMAIL" != *"+$OWNER_EMAIL" ]]; then
      echo "::error::$sha — authored by $AUTHOR, not the owner ($OWNER_EMAIL): $TITLE"
      FAIL=1
    fi
    DESCRIPTION=${TITLE#*: }
    if [ ${#DESCRIPTION} -gt 100 ]; then
      echo "::error::$sha — description > 100 chars (${#DESCRIPTION}): $TITLE"
      FAIL=1
    fi
    SCOPE=$(echo "$TITLE" | sed -nE 's/^[a-z]+\(([^)]+)\)!?: .*/\1/p')
    if [ -n "$SCOPE" ] && ! grep -qx "$SCOPE" <<<"$SCOPES"; then
      echo "::error::$sha — scope '$SCOPE' is not in docs/commit-rules.md § Scope: $TITLE"
      FAIL=1
    fi
    TRIMMED=$(echo "$TITLE" | sed 's/[[:space:]]*$//')
    if [[ "$TRIMMED" == *. ]]; then
      echo "::error::$sha — title ends with period: $TITLE"
      FAIL=1
    fi
    BODY_LINES=$(echo "$BODY" | { grep -v '^$' || true; } | { grep -v '^BREAKING CHANGE:' || true; } | wc -l)
    if [ "$BODY_LINES" -gt 5 ]; then
      echo "::error::$sha — body > 5 lines ($BODY_LINES): $TITLE"
      FAIL=1
    fi
    if echo "$BODY" | grep -i '^Co-Authored-By:' >/dev/null; then
      echo "::error::$sha — contains Co-Authored-By trailer: $TITLE"
      FAIL=1
    fi
  fi
done
if [ "$FAIL" -eq 0 ]; then
  echo "✅ All commits pass the commit rules."
fi
exit "$FAIL"
