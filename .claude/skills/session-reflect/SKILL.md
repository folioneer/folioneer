---
name: session-reflect
description: End-of-session maintenance — audits recent work (git log, memory-file mtimes, CLAUDE.md diff) for rules earned, contradicted, or now surplus, then proposes promote / remember / trim / skip decisions for CLAUDE.md. Output-only — the user confirms each and the main agent applies. Trigger when wrapping up a session ("done for today", "/exit") or after a notable stretch of changes; not for start-of-session triage — use /whats-next for that.
tools: Bash, Read, Grep
---

# Skill — `session-reflect`

A short end-of-session check: did anything emerge worth promoting, contradicting, or trimming?

Output-only. The skill proposes; the user confirms each edit, and the main agent applies it. The skill never auto-writes to `CLAUDE.md` (memory entries follow the existing auto-memory write path).

---

## When to use

- **End of session** — user signals `good night`, `done for today`, `next task tomorrow`, `/exit`, or otherwise wraps the day.
- **After a notable stretch** — reviewer findings that produced new conventions, repeated user corrections, an emergent pattern across multiple PRs.
- **Periodic** — once a week if not naturally triggered, or whenever CLAUDE.md crosses its length budget (~250 lines).

**Bypass the skill entirely** when the session was routine and no rule shifts are visible in the artifacts (Step 1). Just say "No CLAUDE.md changes — routine session." and stop.

---

## Required tools

`Bash`, `Read`, `Grep`.

---

## Compact-resilience

This skill is designed to survive an in-session `/compact`. After a compact, Claude loses verbatim user quotes and the texture of earlier reviewer findings, but persistent artifacts remain. **Signal priority order:**

1. `git log --since="1 day ago"` and `git log main..HEAD` — commits authored this session (never lost).
2. `ls -lt {memory_dir}/feedback_*.md` — memory entries added/updated (file mtimes reveal session activity). Skip if the project has no auto-memory directory.
3. `git diff main..HEAD CLAUDE.md` — rules already added/changed this session.
4. `docs/techdebt.md` entries dated today — tech-debt filed (if the project tracks tech debt there).
5. Conversation context (verbatim quotes, reviewer texture) — **supplement only**, not required.

Throughout, `main` stands for your repo's default branch — substitute if it differs (`master`, `trunk`, …).

If a candidate is only visible in conversation context (signal 5) and not in any persistent artifact, treat it as low-confidence — likely belongs in `Remember`, not `Promote`.

---

## Execution

### Step 1 — Gather artifact signals

Run in parallel (substitute for your environment: `{memory_dir}` → your auto-memory directory from context, typically `~/.claude/projects/<project-slug>/memory/`; `main` → your repo's default branch; `{today}` → today's date as `YYYY-MM-DD`. Skip the memory signal if the project has no auto-memory):

```bash
wc -l CLAUDE.md
```

```bash
git log --oneline --since="1 day ago"
```

```bash
git diff --stat main..HEAD CLAUDE.md
```

```bash
ls -lt {memory_dir}/ 2>/dev/null | head -10
```

```bash
grep -n "^## {today}" docs/techdebt.md 2>/dev/null
```

From the conversation (supplement only): user preferences expressed verbatim ("I will NEVER…", "from now on…", "always…", "stop doing…"), reviewer findings applied vs rejected, repeated decisions across multiple tasks.

### Step 2 — Bucket each candidate

For every signal, decide:

- **Promote (→ CLAUDE.md)** — high bar; rule meets promotion criteria:
  1. Appeared in this session AND at least one prior session (`git log --grep="<keyword>"` for related commits), AND
  2. Project-wide generality (binds every future session, not just one feature's behavior).
- **Remember (→ `memory/feedback_*.md`)** — low bar; signal worth keeping but premature for CLAUDE.md: single-session preference, behavior-only, or not yet re-applied across sessions.
- **Trim (→ CLAUDE.md)** — existing rule was contradicted this session, redundant with another rule, replaced by a hook/lint/agent, or unexercised this session despite clear opportunities. Evidence required.
- **Skip** — routine signal, no action. Default outcome.

If your project sets a CLAUDE.md length budget (this project targets ~250 lines) and CLAUDE.md is above it, additionally include a per-section line count (`grep -n '^## ' CLAUDE.md`) and flag the largest sections as trim candidates even without specific contradiction signals.

### Step 3 — Emit proposals

Emit the result per `## Output format` below: the routine-session one-liner when every signal buckets to `Skip`, otherwise the proposal table closed with `Confirm Promote/Trim entries to apply.`

### Step 4 — Hand off for application

This skill is output-only: it does not edit `CLAUDE.md` or write memory itself. After the user accepts or rejects each row, the **main agent** applies the confirmed entries:

- **Promote** → main agent edits CLAUDE.md (locate the appropriate section; add as a new bullet or sub-bullet). Use the established section structure; don't invent new top-level sections without flagging it.
- **Trim** → main agent edits CLAUDE.md (delete the rule, or consolidate with another).
- **Remember** → main agent uses the auto-memory write path (`{memory_dir}/feedback_*.md` + `MEMORY.md` index update) — same mechanism the auto-memory system uses every session. If the project has no auto-memory directory, Remember degrades to propose-only — surface the rule for the user to record manually.

---

## Output format

When every signal buckets to `Skip`:

> No CLAUDE.md changes — routine session.

Otherwise, a single proposal table — keep each cell terse (an evidence cell is a reference, not a sentence; ≤40 chars):

| #   | Bucket   | Item                      | Evidence                       |
| --- | -------- | ------------------------- | ------------------------------ |
| 1   | Promote  | One-line rule statement   | commit sha + prior-session ref |
| 2   | Trim     | Rule to remove or rewrite | contradiction/redundancy cited |
| 3   | Remember | Rule going to memory only | why premature for CLAUDE.md    |

Close with: `Confirm Promote/Trim entries to apply.`

---

## Critical rules

1. **Output-only** — propose, don't auto-edit CLAUDE.md; the main agent applies confirmed rows (Step 4). The user confirms each Promote/Trim.
2. **Specific evidence required** — every proposal cites something concrete from this session (a commit, a memory entry, a reviewer finding, a verbatim user quote). No theoretical "we should add this".
3. **Brief output** — one line when nothing applies. Short table otherwise. No prose preamble.
4. **Honor promotion criteria** — Promote proposals require evidence in ≥2 sessions AND project-wide generality. Single-session signals go to Remember.
5. **Compact-resilient signals only for Promote** — if the only evidence is conversation context (no commit, no memory entry, no CLAUDE.md diff), downgrade Promote proposals to Remember. A rule promoted to CLAUDE.md must be visible in persistent artifacts so a future audit can verify its origin.
6. **Length budget as proactive trim signal** — when CLAUDE.md exceeds the project's length budget (this project targets ~250 lines), surface a per-section line distribution and propose trim candidates even without contradiction evidence.

---

## Notes

This skill complements `/whats-next` (start-of-session: triage pending work). Together they bracket a day: pick task → execute → reflect.

The four memory categories (user / feedback / project / reference) are unchanged — `Remember` outputs typically land as `feedback` entries. The auto-memory system's normal write criteria apply.
