#!/usr/bin/env bash
# publish-release.sh — the owner publishes the newest draft release, after checking it.
#
# Run by `just publish-release`, in the owner's own terminal: the agent never publishes
# (CLAUDE.md rule 1; `.claude/settings.json` denies this recipe and this script).
# It refuses without a terminal, when the draft's tag is not the newest `v*.*.*` tag on
# main, or when that tag's release workflow did not end green on the tag's commit; then
# it shows the draft and publishes only on an explicit yes.
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/usage-log.sh"
cd "$(dirname "${BASH_SOURCE[0]}")/.."

fail() {
  echo "publish-release: $1" >&2
  exit 1
}

[[ -t 0 ]] || fail "needs a terminal to confirm: run it yourself, never from an agent"

tag=$(gh release list --limit 20 --json tagName,isDraft --jq '[.[] | select(.isDraft)][0].tagName // ""')
[[ -n "$tag" ]] || fail "no draft release"

git fetch -q origin main
newest=$(git ls-remote --tags --refs --sort=-v:refname origin 'v*.*.*' | head -n 1 | sed 's|.*refs/tags/||')
[[ "$tag" == "$newest" ]] || fail "the draft is $tag, the newest tag is ${newest:-none}"

# The commit the remote tag names, an annotated tag peeled.
tag_sha=$(git ls-remote origin "refs/tags/$tag^{}" | cut -f1)
[[ -n "$tag_sha" ]] || tag_sha=$(git ls-remote origin "refs/tags/$tag" | cut -f1)
git merge-base --is-ancestor "$tag_sha" origin/main 2>/dev/null || fail "$tag is not on main"

IFS=$'\t' read -r conclusion run_sha url < <(gh run list --workflow release.yml --branch "$tag" \
  --event push --limit 1 --json conclusion,headSha,url \
  --jq '.[0] // {} | [(.conclusion // "" | if . == "" then "unfinished" else . end), (.headSha // "none"), (.url // "none")] | @tsv')
[[ "$run_sha" == "$tag_sha" ]] || fail "no release workflow run on $tag's commit"
[[ "$conclusion" == "success" ]] || fail "the release workflow on $tag ended $conclusion: $url"

echo "Draft $tag — release workflow green: $url"
echo
gh release view "$tag" --json name,body --jq '"\(.name)\n\n\(.body)"'
echo
read -r -p "Publish $tag? [y/N] " answer
[[ "$answer" == "y" || "$answer" == "Y" ]] || fail "not published"

gh release edit "$tag" --draft=false --latest
echo "Published $tag."
