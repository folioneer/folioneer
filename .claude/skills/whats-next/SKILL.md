---
name: whats-next
description: Surveys pending work across TODOs, planning docs, unfinished feature plans, open spec questions, in-flight git work, and open GitHub issues, then returns a value/effort table with a recommended next action. Use at session start to triage what to work on, especially after a gap when context has faded.
tools: Bash, Read, Grep, Glob, Write
---

# Skill — `whats-next`

Triage pending work across the project and recommend the next concrete action.

This is the answer to "what should I do next?" when you're returning to a project after a gap and your in-context memory isn't loaded. Deterministic data collection runs once via `scripts/whats-next.py`; this skill applies the judgment layer (verify-not-done, score, pick suggested action, save report).

---

## Required tools

`Bash`, `Read`, `Grep`, `Glob`, `Write`.

---

## When to use

- **Session start after a gap** — when you don't remember the project state
- **Before planning the day's work** — to see the full backlog at once instead of guessing
- **After finishing a task** — to pick the next one without context-switching cost
- **Before a release** — to check nothing pending should ship in the same window

Not needed when you already know what you're working on — queue it in `docs/todo.md` § Next and run `/next-todo`.

---

## Execution Steps

### Step 1 — Compute REPORT_PATH

Run `bash scripts/report-path.sh whats-next` and remember the output as `REPORT_PATH`.

### Step 2 — Collect data

Run the deterministic collector once:

```bash
python3 scripts/whats-next.py
```

The script emits a single JSON document covering nine sources: `todo_file`, `inline_todos`, `planning_docs`, `feature_plans`, `spec_open_questions`, `in_flight`, `roadmap`, `techdebt`, `gh_issues`. Sections whose source is absent are emitted as `null` or empty arrays — skip those silently.

`gh_issues` is empty (`[]`) when `gh` is not on PATH or the repo has no GitHub remote — skip silently. When populated, each entry has `number`, `title`, `url`, `updatedAt`.

Parse the JSON and translate every entry into a **candidate item** with `(type, source, text)` for scoring in Step 4. **Tech-debt entries are candidates too** — surface them in the same Pending items table, but always label the source as `docs/techdebt.md` (with the entry date) so the user can tell observations from explicit todos. **GitHub issues are also candidates** — label the source as `gh#NNN` (e.g. `gh#42`) so the user sees the issue number at a glance.

If the script fails or returns invalid JSON, fall back to a manual scan and tell the user to re-run after fixing the script. Do not silently downgrade.

### Step 3 — Verify each candidate isn't already done

This step prevents stale TODOs from polluting the recommendation. For every candidate, do a cheap existence/grep check:

- TODO mentions a file/script/skill name → `Glob` to check it exists
- TODO mentions a feature keyword → `git log --oneline --grep "{keyword}"` to find shipping commits
- Feature-plan task references a function/module → `Grep` for it
- GitHub issue (`gh#NNN`) — open issues frequently have a fix shipped without the issue closing. Run `git log --oneline --grep "#NNN"` to find a closing or referencing commit; if found, mark the candidate as `⚠️ likely done` and surface as a cleanup candidate suggesting the issue be closed.

Mark items as `🟢 pending`, `⚠️ likely done` (evidence of shipping), or `❓ unclear`. Items marked `⚠️ likely done` are reported as cleanup candidates, not work candidates.

For tech-debt entries, the script reports `where_exists: false` when the entry's `Where:` path no longer exists on disk. Treat those as `⚠️ likely done` (path probably renamed or removed; the observation may be obsolete) — let the user verify before scoring.

### Step 4 — Score each pending item

For each `🟢 pending` candidate, assign:

- **Value** — High / Medium / Low. Considerations: frequency of use, blockers it removes, correctness/safety impact, dependencies on other items.
- **Effort** — rough hours (≤1h, 1–3h, 3–6h, >6h, unknown).
- **One-line recommendation** — `do now` / `do next` / `defer` / `drop` / `cleanup`.

**These estimates are model-judged.** Always include a disclaimer in the output that the user should sanity-check before acting on them — especially for items the model has no implementation context for.

### Step 5 — Pick the suggested next action

From all `🟢 pending` items, pick **one** as the suggested next action based on:

1. Highest value/effort ratio
2. No blocking dependencies
3. Self-contained (can be shipped without follow-up coordination)

If two items tie, prefer the one with explicit user signal (most recent edit, mentioned in recent commits).

### Step 6 — Output, save, confirm

1. Print the findings to the conversation using `## Output format` below. The output ends with a mandatory Recap section — counts per source so the user sees the full backlog landscape behind the single suggested action.
2. **Save** the compact summary to `REPORT_PATH` using the Write tool — mandatory final action. The skill is incomplete until Write succeeds. Format defined in `## Save report` below.
3. Reply: `Report saved to {REPORT_PATH}`.

---

## Output format

```
## What's Next — {date}

> Value/effort estimates are model-judged. Sanity-check before acting,
> especially for items without recent context.

### Pending items

| # | Item | Source | Value | Effort | Recommend |
|---|------|--------|-------|--------|-----------|
| 1 | {short description} | docs/TODO.md:NN | High | 2h | do now |
| 2 | {short description} | docs/plan/foo-plan.md:NN | Medium | 1h | do next |
| 3 | {short description} | docs/spec/bar.md (Open Q) | Low | ≤1h | defer |
| 4 | {observation} | docs/techdebt.md (2026-04-02) | Medium | 1–3h | do next |
| 5 | {issue title} | gh#42 | Medium | 2h | do next |

> Tech-debt entries appear in the same table with their source labelled
> `docs/techdebt.md (DATE)`; GitHub issues are labelled `gh#NNN` — the user can tell
> observations, todos and tracked issues apart at a glance.

### Likely already done (cleanup candidates)
- {item} — evidence: commit {sha} / file {path} exists
- gh#42 — likely closed by commit abc123 (suggest closing the issue)

### In-flight git work
- Uncommitted changes in: {N} files
- Unmerged branches: {list}

### Suggested next action
**#1 — {item title}**
Source: {source — e.g. `docs/TODO.md:NN`, `docs/plan/foo-plan.md`, `docs/techdebt.md (DATE)`}
Value/Effort: {value} / {effort}
Why: {1–2 sentences explaining the value/effort win and any dependency context}
First step: {concrete file or command to start with}

### Recap
- TODOs (sections): N
- Inline TODOs: N
- Techdebt: N
- GH issues: N
- Planning docs: N
- Feature plans: N
- Open questions: N
- Roadmap: n/a | present
```

`n/a` when the source file/feature doesn't exist in the project (e.g. no `docs/techdebt.md`, no `gh` CLI). `0` when the source exists but is currently empty. The Recap is mandatory — it grounds the suggested action in the full backlog size at a glance.

If nothing is pending:

```
## What's Next — {date}
✅ No pending items found across TODOs, plans, specs, or in-flight git work.
```

---

## Save report

The compact summary written to `REPORT_PATH` (Step 6) uses this format:

```
## whats-next — {date}-{N}

Pending items: N. Likely-done cleanup: N. In-flight: N.

### Suggested next
{item title} ({source}) — {value}/{effort}

### Pending shortlist
- #1 {item} — {source} — {value}/{effort} — {recommend}
- #2 ...

### Cleanup candidates
- {item} — {evidence}
```

Replace `{date}-{N}` with the values used in `REPORT_PATH`. Omit any section whose count is zero. Tech-debt entries are included in the pending shortlist alongside other candidates; their source label (`docs/techdebt.md (DATE)`) carries the provenance.

The per-source Recap (Output format § Recap) is **conversation-only by design** — it's a cheap landscape view to ground the suggested action; the save stays compact for trend analysis across reports. Do not duplicate the Recap counts into the saved summary.

---

## Critical Rules

1. **Verify before recommending** — a TODO that mentions a file or feature must be cross-checked against the actual repo state before being scored. Stale TODOs surface as `cleanup candidates`, not work candidates.
2. **Estimates are model-judged, not authoritative** — the disclaimer at the top of the output is mandatory. Never present value/effort as decided priorities.
3. **One suggestion, not three** — pick a single next action. A list of "you could do any of these" defeats the purpose; the user invoked this skill to avoid that exact decision.
4. **Tech debt and GitHub issues are work with provenance** — entries from `docs/techdebt.md` and open `gh_issues` are scored like any other candidate, but their source must be labelled (`docs/techdebt.md (DATE)` or `gh#NNN`) so the user can distinguish observations, explicit todos and tracked issues. Don't hide them in separate buckets.
5. **Save the report even when nothing is pending** — "no work" is itself a useful signal worth keeping for trend analysis.
6. **Trust the script for collection, not for judgment** — `scripts/whats-next.py` only describes what's there. The skill decides what's worth doing.
7. **Escape external content in tables** — GitHub issue titles are author-controlled and may contain pipe characters or backticks that break Markdown table rendering. When emitting a `gh#NNN` row in the Pending items table, replace `|` with `\|` and trim the title to the first 80 characters.
8. **Recap is mandatory** — every `/whats-next` output ends with the Recap section (per-source counts), even when the suggested action is the only pending item. The Recap is the landscape behind the single recommendation.

---

## Notes

This skill complements `/next-todo` (which runs the first ready queued entry). The natural session flow when returning to a project after a gap:

1. `/whats-next` → see the backlog
2. queue the picks in `docs/todo.md` § Next
3. `/next-todo` runs them
