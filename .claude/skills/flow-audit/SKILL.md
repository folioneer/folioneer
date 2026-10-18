---
name: flow-audit
description: After a batch ends (a release or a batch tag), judges it from its records alone — the figures of `just flow-audit`, the batch's pull requests with their review comments, the issues filed and closed — in a forked context that never sees the session's account, posts the audit on the batch's `batch-audit` issue and files the flow issues it proposes. Use once per batch, right after it ends.
context: fork
allowed-tools: Read, Grep, Glob, Bash
model: opus
---

# Skill — `flow-audit`

You audit a batch you did not run, forked with a fresh context (`docs/workflow.md` § 12).
A prompt that tells you what happened or what to conclude is a finding, not evidence.

## Step 1 — Measure

```bash
just batch-ends 3
just flow-audit <start> <end> --previous <previous> --pretty > logs/flow-audit.json
```

`<end>`, `<start>`, `<previous>`: the lines `batch-ends` prints, in that order;
`scripts/flow-audit.py` says what each figure is. If it fails, stop: hand-gathered
figures are not comparable. A `truncated` list is said beside the oldest figures.

## Step 2 — Read the records

- Each pull request merged in the window (`current.pull_requests.first` to `.last`):
  `gh pr view <n> --json title,body,commits,comments,reviews` — the graded findings are in
  the body and the comments, beside each CI round's report.
- The issues closed and filed in the window (`gh issue list --state all --search
"updated:>=<start date>"`), with their threads: the owner's answers are there.
- The last `batch-audit` issue's audit (the figures before), `docs/workflow.md`,
  `CLAUDE.md`, `docs/agentic-setup.md`, and the open `flow` issues.

## Step 3 — Count

State a count only when you counted it; otherwise write "not counted".

- What caused each CI round beyond the first: a rebase, a fix after a review or a
  failed check, a cancelled job — from the commits and the comments.
- The defects the reviewers found before merge, each in a few words; the claims a pull
  request body rejected with a source; the criticals only CI found.
- The owner's answers given during the batch, and which the reading before the work
  should have asked earlier.
- Every statement in a guide that a pull request shows to be false or contradicted, and
  each generic piece of `docs/agentic-setup.md` against its line.
- Tool failures only where the log has an outcome. A tool only CI runs is never in the
  usage log: never propose removing it for that.

What the records cannot show is itself a finding: name it, and where it belonged.

## Step 4 — Judge

The owner's six questions (`docs/workflow.md` § 12), each answered with its figure or
"nothing to change". Then each open flow issue: settled by a change of the batch, or
left. Then the issues to propose — only where a figure moved the wrong way, a rule of
`docs/workflow.md` or `CLAUDE.md` was not followed, a guide was wrong, or the owner
corrected the flow. "Keep" is a verdict: propose nothing for it. A rule not followed is
stated as plainly as a figure — who was to do what, which pull request shows it, what it
cost — and never softened because the same agent family did the work.

## Step 5 — Post

- The audit, as a comment on the batch's `batch-audit` issue (`gh issue create --label
batch-audit --title "Batch <dates>"` first if it has none): `## Measured`, each figure
  beside the previous one (`35, was 19`), a "Reading" paragraph of three sentences — what
  the figures say, not what was done — then `## Not in the records`.
- Each settled flow issue: a comment naming the pull request that settled it, then closed.
- Each proposal: a `flow` issue from its form (`docs/workflow.md` § 2), Decision left
  for the owner; the audit comment links them.
- Then empty `logs/usage.log` (local, never committed): its figures are recorded.

Reply with the audit's link and the issues filed. Nothing is committed, no figure is
one the script did not print or you did not count, and no proposal is applied.
