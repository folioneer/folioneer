---
name: flow-audit
description: After a release, measures how the batch moved (pull requests, time to merge, CI rounds and failures, what the agent used) against the release before, rewrites the measured sections of docs/flow.md, and proposes flow entries only where a figure moved or a rule was not followed. Use once per release, right after it is published.
---

# Skill — `flow-audit`

`docs/flow.md` says what the workflow owes the two people who run it. Its figures are
gathered here, the same way after every release, so the flow is improved on figures and
not on impressions. The script counts; the skill judges. It decides nothing for the
owner: it proposes entries, which the owner queues or declines.

---

## Required tools

`Bash`, `Read`, `Edit`.

---

## When to use

- Right after a release is published, in the session that ran its batch: the judging
  needs that session's context.
- Not during a batch: a half-run batch measures nothing comparable.

---

## Execution steps

### Step 1 — Measure

```bash
python3 scripts/flow-audit.py <previous-release> <this-release> --previous <the-one-before> --pretty
```

For the batch that led to `v0.6.0`: `python3 scripts/flow-audit.py v0.5.0 v0.6.0
--previous v0.4.0 --pretty`. One JSON document, `current` beside `previous`:

- `pull_requests` — merged, first and last number, those closed without merging, lines
  added, mean and median minutes from opening to merging, the count by commit type.
- `ci` — rounds (one pushed commit of a branch), branches, rounds beyond the first,
  failures by workflow, the median duration of each workflow's green runs.
- `truncated` — what GitHub's lists cut short. When it names anything, the oldest figures
  are incomplete: say so beside them, or raise the limits in the script first.

Two bounds to keep in mind when reading: runs are matched to a pull request by its branch
within the window, so one opened before the window loses its earlier rounds; a failure is
counted per run, so failures can exceed rounds.

If the script fails, say so and stop: figures gathered by hand are not comparable.

### Step 2 — Count what the script cannot

From the session's transcript and memory notes, for the batch only:

- what caused the rounds beyond the first: a rebase after another merge, a fix after a
  review or a failed check, the usage limit;
- real defects found by the reviewers before merge, and by challenging an entry before
  starting it — each named in a few words;
- what the agent invoked itself: skills, local agents, recipes, native commands where a
  recipe exists, scripts run directly;
- what exists and was not used: skills, agents, recipes, scripts.

State a count only when it was counted; otherwise write "not counted".

### Step 3 — Rewrite the two measured sections

In `docs/flow.md`, replace `## Measured — the <previous> batch` and `## Used and not
used — the <previous> batch` with this batch's, same tables, each figure beside the
previous one (`33, was 27`). Keep the "Reading" paragraph to three sentences: what the
figures say, not what was done.

### Step 4 — Re-read the open entries

For every `FLOW-NNN` and every todo or tech-debt entry the file holds:

- **settled** by a change of the batch → remove the entry;
- a **"keep, and re-measure"** verdict → state the new figure in it, and keep or change
  the verdict on that figure;
- a **proposal** neither queued nor declined → leave it, unless a figure now argues
  for or against it: then say so in its text.

### Step 5 — Propose entries

Add a `FLOW-NNN` entry (next free number, stated at the top of the file) only where a
figure moved the wrong way, a rule of `docs/workflow.md` or `CLAUDE.md` was not
followed, or the owner corrected the flow during the batch. Each entry in the file's
format: Kind, Observed (with its figure), Proposal or Verdict, Costs and what it
protects. "Keep" is a verdict; where nothing needs changing, propose nothing.

### Step 6 — Hand over

One pull request, docs only. Its closing brief lists the figures that moved and the
entries proposed, for the owner to queue or decline in the next batch's opening block.

---

## Rules

1. **The script counts, the skill judges.** No figure in the measured sections that the
   script did not print or Step 2 did not count.
2. **Beside the previous release.** A figure alone says nothing; every one is shown with
   the one before.
3. **Quality and speed together.** An entry that buys speed says what it risks; one that
   buys safety says what it costs.
4. **The owner decides.** Never edit § Next; never apply a proposal in the audit's own
   pull request.
