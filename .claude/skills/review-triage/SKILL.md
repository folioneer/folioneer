---
name: review-triage
description: Triages reviewer-* findings against the (a)/(b)/(c) per-task discipline before any are applied. Reads `.review/` reports, grades each finding, emits a per-row Follow-up table, and applies the Workflow C policy per grade without halting. Applied after every reviewer batch under Workflow C; also usable standalone after ad-hoc reviewer runs. Routes (b) rows to `/techdebt` — does not replace it.
tools: Read, Glob, Bash, Write
---

# Skill — `review-triage`

Reviewer-\* agents persist their full output to `.review/{slug}-DATE-NN.md` (via `bash scripts/review-path.sh`) so this skill can triage the complete report, not the reviewer's terminal summary.

---

## Required tools

`Read`, `Glob`, `Bash`, `Write`.

---

## When to use

- **After every reviewer-\* agent batch** — the standard auto-invoke point. A "batch" is all reviewer-\* agents run since the last commit; the reset boundary is the commit that follows the batch.
- **Before applying any finding** — even on a single-reviewer run.
- **Whenever you'd be tempted to silently apply or silently defer a finding** — exactly the failure mode this skill prevents.

Not needed if all reviewers in the batch returned clean (no findings) — the skill detects this and exits.

---

## Execution Steps

### Step 1 — Compute REPORT_PATH and locate reviewer reports

Run `bash scripts/report-path.sh review-triage` and remember the output as `REPORT_PATH`.

Locate reviewer reports newer than the HEAD commit (the reset boundary):

```
bash scripts/list-fresh-reviews.sh
```

Output is one report path per line (sorted), empty if no fresh reports. If empty → emit the "no reports" message in `## Output format` and exit (skipping Steps 2–5).

### Step 2 — Parse findings

For each matched report file, `Read` it and extract every finding marker:

- `🔴` critical
- `🟡` warning
- `🔵` suggestion
- `ℹ️ Pre-existing tech debt` → EXCLUDE (the reviewer's own convention already routes these via `/techdebt`)

Tally combined count across all files. If 0 → emit clean exit message and exit.

### Step 3 — Challenge, then grade

For each finding, answer these four questions in order. The answers DETERMINE the grade — there is no separate "challenge" step.

**Q1: Introduced by the current branch diff?** Verify via `bash scripts/branch.sh diff {file}` for the cited file/line.

- Yes → candidate for **(a)**. Continue to Q3.
- No → it's pre-existing. Go to Q2.

**Q2: If pre-existing, is it boyscout-eligible?** All three must hold:

- Inside the PR scope (file appears in `git diff {base}..HEAD` — the cumulative branch diff against the PR's base, NOT a single commit's diff; see "Scope is PR, not commit" in Notes)
- Small (≤~10 LOC of fix diff — measured as the lines this fix would add+delete, not the blast radius)
- Mechanical (rename, import update, signature swap — no fresh design judgment)
- All yes → **(a)**.
- Any no → **(b)**.

**Q3: Does it require design judgment, OR multi-file fanout outside the PR scope?**

- Yes → **(b)**.
- No → continue to Q4. **Note**: multi-file fanout INSIDE the PR scope is still (a) if mechanical — the PR is the natural fold boundary. Splitting "fix the wording" from "ship the wording" inside the same PR produces churn without value.

**Q4: Is the finding empirically wrong, scope-creep, YAGNI, or stylistic-without-rule-basis?**

- Yes → **(c)** (this OVERRIDES any prior tentative (a) or (b) — a diff-introduced finding can still be a false positive). Sub-split: recurring across multiple files in this batch OR rationale would bind future sessions / repo-wide → **(c) pattern**; otherwise → **(c) one-off**.
- No → keep the prior tentative grade ((a) or (b) from Q1–Q3).

**Default when ambiguous:** prefer **(c) one-off** over (c) pattern. Pattern-level rejections drag the user into ADR territory and should be rare.

### Step 4 — Emit triage table

Format per `## Output format` below. Every row carries:

- **Source** — reviewer name
- **File:Line** — from the report's location reference
- **Finding** — severity emoji + one-line summary
- **Grade** — (a), (b), (c) one-off, or (c) pattern
- **Rationale** — which question above settled the grade; one sentence
- **Follow-up** — mechanical action from the table in `## Follow-up shapes` below

### Step 5 — Apply the policy per grade

No halt. Under Workflow C (`docs/workflow-c.md` § 7) every grade has a mechanical
follow-up: (a) fix in the PR, (b) `TD-NNN` entry in `docs/techdebt.md`, (c) one-off
inline `// <reviewer> FP: <reason> — see PR #NN`, (c) pattern → edit the reviewer
prompt in the same PR. A `[DECISION]` critical becomes an open question on the entry.
The table goes into the PR body; the human reads it there, never in a prompt.

### Step 6 — Save report and signal

`Write` the triage table + decisions to `REPORT_PATH` (format in `## Save report`). Reply with:

`Triage report saved to {REPORT_PATH}. Main agent: apply each row's Follow-up.`

Do not continue past this reply — return control to the main agent for Follow-up execution.

---

## Follow-up shapes

Per per-task rule 5's tracking conventions:

| Grade       | Follow-up template                                                                                                                                                          |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| (a)         | `Apply fix; commit captures it.` Append `Add Addresses <source>: <gist> to commit body only if <source> is invisible on PR page (e.g. local reviewer-agent run pre-merge).` |
| (b)         | `/techdebt where=<path> obs="<one-line observation>" found-by=<reviewer> severity=<emoji>`                                                                                  |
| (c) one-off | `Add inline comment at <file:line>: // <source> FP: <reason> — see PR #NN`                                                                                                  |
| (c) pattern | `Edit the reviewer prompt in the same PR so the finding stops recurring; note it in the PR body.`                                                                           |

---

## Output format

```
## /review-triage — {N_total} findings from {M} reviewers

> Triage applies the per-task (a)/(b)/(c) challenge discipline before
> any finding is applied. (b) and (c) rows require user confirmation.

Reviewers in batch: {comma-separated names}
Source files: {comma-separated paths under .review/}

| # | Source | File:Line | Finding | Grade | Rationale | Follow-up |
|---|--------|-----------|---------|-------|-----------|-----------|
| 1 | reviewer-backend | src/foo.rs:42 | 🔴 unwrap() in prod path | (a) | Introduced by diff; mechanical fix (Q1 yes) | Apply fix; commit captures it |
| 2 | reviewer-arch | src/foo.rs:1-50 | 🔴 cross-context import | (b) | Pre-existing, 8 files, multi-file refactor (Q2 fails locality) | /techdebt where=src/foo.rs:1-50 obs="Cross-context import" found-by=reviewer-arch severity=🔴 |
| 3 | reviewer-frontend | src/qux.tsx:30 | 🟡 i18n key fallback | (c) one-off | Key is fallback for unreachable branch (Q4: empirically wrong) | Add inline at src/qux.tsx:30: // reviewer-frontend FP: unreachable fallback — see PR #NN |
| 4 | reviewer-frontend | src/quux.tsx:1-200 | 🟡 component >200 LOC | (c) pattern | Convention not codified (Q4: stylistic without rule basis); recurs in 3 components | Edit reviewer-frontend's prompt so the 200-LOC heuristic stops firing; note it in the PR body |

Main agent: apply each row's Follow-up in order. Do NOT apply any finding outside this table.
```

When the batch is clean (no reports OR all reports had 0 findings):

```
## /review-triage — clean

{No reviewer reports since last commit | All N reviewer(s) in batch returned no findings}. No triage required. Main agent: proceed.
```

---

## Save report

The triage report at `REPORT_PATH`:

```
## review-triage — {date}-{NN}

Timestamp: {ISO 8601 UTC, e.g. 2026-05-19T14:23:00Z}
Sources: {comma-separated paths from Step 1's list-fresh-reviews.sh output}
Reviewers: {comma-separated names}
Findings: {N_total} ({a_count} (a), {b_count} (b), {c1_count} (c) one-off, {cp_count} (c) pattern)
User decision: {Proceed | Adjust}

### Triage table
[same table as terminal output]
```

When the batch is clean, the saved report is one line: `## review-triage — {date}-{NN}\n\nClean batch. No findings.`

---

## Critical Rules

1. **Challenge is the grading test, not a separate step.** Each finding's grade comes from answering Q1–Q4 in Step 3. "Looks like (a)" is not a grade — name the question that settled it in the Rationale column.
2. **Record (b) and (c) in the PR body.** Per-task rule 5: "don't silently defer or silently apply" — the table in the PR body is the record; no prompt to the human.
3. **(c) pattern is rare by default.** Default to (c) one-off when in doubt. Pattern-level rejections only fire when the rationale genuinely binds future sessions or applies repo-wide.
4. **Pre-existing tech-debt findings are excluded.** Reviewer's own `### ℹ️ Pre-existing tech debt` section already routes those via `/techdebt`; do not re-process here.
5. **Skill is output-only.** The tools grant excludes any slash-command invocation tool by design — it has no way to invoke `/techdebt`, `/adr-writer`, or any other skill. The table is the artifact; the main agent owns execution of each Follow-up.
6. **Mechanical Follow-ups.** Each Follow-up cell must be specific enough to execute without further thought — concrete file:line, exact `/techdebt` arg shape, exact inline comment text. "Apply fix" qualifies; "consider applying" does not.
7. **No silent batch-accept.** Even when 12 findings are the same lint warning, each gets a row with its own grade (or one row with `Affects: <list>` if truly identical — the grade is still explicit).

---

## Notes

**Scope is PR, not commit.** The (a)/(b)/(c) decision treats the PR (cumulative diff against the PR's base branch) as the scope unit. A wording fix that fans across 10 files all touched by the same PR is (a) if mechanical — fold it into the PR, don't defer. The boyscout test in Q2 and the fanout test in Q3 both use PR scope. Per-commit framing would create artificial splits ("fix the wording" vs "ship the wording" as separate PRs) that produce churn without value.

The (a)/(b)/(c) discipline this skill encodes is per-task rule 5 in the downstream project's CLAUDE.md (§ Per-task Discipline). The skill is self-contained — it works in projects whose CLAUDE.md doesn't carry the rule, because the grading axes live in Step 3 above.

Under Workflow C (`docs/workflow-c.md` § 7), after a reviewer batch reviewer-\* agents save reports to `.review/`; the next checkbox is `/review-triage`; the agent applies the policy per grade — no halt — and records the table in the PR body.

If the user picks "Adjust grades" in Step 5, the skill exits without applying anything; the user responds in chat with grade corrections, then re-runs the skill (or the main agent applies the corrected grades manually). The consolidated halt is the design trade — per-row prompting would create 4-12 questions per batch.

Reviewers run sequentially within a batch (the report-path naming + `.review/` folder are not race-safe under parallel invocation — see the concurrency note in `scripts/review-path.sh`).
