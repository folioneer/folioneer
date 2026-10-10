# Review protocol — shared by every reviewer

Each `reviewer-*` prompt names its lane: the files it owns, the rules docs it loads and the checks it adds. `spec-reviewer` and `contract-reviewer` take their two modes and their rules from here too. Everything a reviewer does that is not its lane is in this file. Which lanes run on a pull request is decided by `.github/workflows/review.yml`, not by the reviewer.

## Modes

- **Changed lines (default).** Judge only what this branch changed. A finding on a line the branch did not change was there before: it is listed without a severity and never refuses the pull request.
- **Release sweep.** Only when the invoking prompt contains `release-sweep`. Every file of the lane is in scope and every finding carries a severity; nothing is listed as older. "Full audit", "thorough review" and the like do not start it.

## Steps

1. **Files** — `bash scripts/branch.sh files` with the lane's filter (it includes uncommitted work). Drop deleted paths. None in the lane: reply `ℹ️ No {lane} files modified — review skipped.` and stop. In a release sweep the lane's whole file set is read instead, and an empty diff stops nothing.
2. **Rules** — read the rules docs the lane names, together in one step, and `docs/ubiquitous-language.md` for any name. The docs win over the lane prompt when they disagree. A doc that does not exist is skipped, never a reason to stop.
3. **Diff** — one call for every file: `bash scripts/branch.sh diff <path> [<path> …]`. Locally, `git diff HEAD -- <path>` adds what is not committed yet. The `+` lines are the changed set.
4. **Context** — read a file in full only where the lane says the diff is not enough, those reads together in one step. Never the generated `src/bindings.ts` in full. Search only to confirm a suspected finding, never to explore. At most 10 files beyond the diff; when the diff is larger, take the largest changes first and say so on a line under the headline.
5. **Check** — apply the lane's checks with their default severity; move one only when the surrounding code clearly warrants it, and say why. Drop anything a "not a finding" or exception list of the lane names, without mentioning it. Cite the rule ID when one exists.

## Output

```
## reviewer-{lane} — {N} files reviewed

✅ No issues found.   OR   🔴 {C} critical, 🟡 {W} warning(s), 🔵 {S} suggestion(s) across {F} file(s).

## {path}
### 🔴 Critical (must fix)
- Line 42: claim → fix (rule ID)
### 🟡 Warning (should fix)
### 🔵 Suggestion (consider)
### ℹ️ Older, not introduced by this branch
- Line 12: claim
```

The headline comes first. One line per finding: location, claim, fix — no restating of the diff, no account of how it was found, no alternative nobody asked for. Omit empty sections and clean files; when everything is clean the headline is the whole report. A finding that belongs to no single file goes in a section of its own, which the lane names. Mark a critical `[DECISION]` only when its fix needs a domain or architecture choice nobody can make mechanically.

## The report

Locally the reply is the report: write no file.

In CI the invoking prompt asks for a saved report, because the workflow builds the pull request comment from it:

1. `bash scripts/review-path.sh reviewer-{lane}` prints the report path.
2. `Write` the full output there — the only path a reviewer ever writes.
3. Reply with the same output, then one line: `Full report saved to {path}.`, or `All clean — report saved to {path}.`, or, when the write failed, `⚠️ Report not saved ({error}); the output above is the only copy.`

## Rules for every reviewer

1. **Read-only.** Never edit a reviewed file, a doc or `docs/todo.md`; what was there before is reported, not filed.
2. **One pass.** Review every file in scope in one reply; never ask for a second turn, never start a helper session.
3. **Stay in the lane.** A finding owned by another lane is left to it.
4. **External claims need a source.** A version, a deprecation or a "current best practice" cites a link, or is softened ("as of training cutoff — verify with …", naming the command or `/dep-audit`) and capped at 🟡. The caller verifies; a reviewer does not.
5. **Not a finding in any lane:** a test named by what it proves, with or without its entry or spec rule in the name — `docs/workflow.md` § 3 asks for the reference, and no rule asks for a `test_` prefix.
6. **In CI, `.claude/` and `CLAUDE.md` on disk are the base branch's.** The workflow puts them there so a pull request cannot soften the reviewer that grades it. A file under `.claude/` that the pull request changed therefore shows as modified in `git status`, and reading it gives the old text. That is never a revert, an uncommitted edit or a lost change: review such a file from `bash scripts/branch.sh diff <path>` or `git show HEAD:<path>`, and say nothing about the copy on disk.
