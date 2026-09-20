---
name: adr-writer
description: Authors and supersedes Architecture Decision Records (docs/adr/*.md). Run when a decision passes the 3-criteria gate (genuinely complex / not obvious from context / costly to reverse). After authoring or superseding, run `adr-reviewer` to validate quality before locking in. Not for tentative decisions — those stay in the spec's `## Open Questions` until ratified.
tools: Read, Glob, Write, AskUserQuestion
model: opus
---

# Skill — `adr-writer`

Produce or supersede an ADR — `docs/adr/{NNN}-{slug}.md` — and keep `docs/adr/README.md` in sync. The ADR captures the _why_ behind a structural choice that the spec cannot carry alone.

---

## Required tools

`Read`, `Glob`, `Write`, `AskUserQuestion`. Interactive — cannot complete in a non-interactive shell.

---

## When to use

- **A decision passes the 3-criteria gate** (see below) — typically flagged via `ADR-SUGGESTED` in a spec's `## Open Questions` by `spec-writer` or `spec-reviewer`
- **Superseding a past decision** — a prior ADR is no longer correct; the new one explains why
- **Indexing existing ADRs** — refresh `docs/adr/README.md` after manual edits or imports

---

## When NOT to use

- **Tentative or unresolved decisions** — keep them in the spec's `## Open Questions` until a final choice is made; ADRs are ratified-only
- **Coding standards or naming preferences** — these belong in convention docs (`docs/backend-rules.md`, `docs/frontend-rules.md`), not ADRs
- **Decisions self-evident from the spec** — if the rule already states the choice and the rationale, an ADR adds noise
- **Reversible single-function choices** — fail criterion 3 ("costly to reverse"); no ADR needed
- **Validating an existing ADR** — use the `adr-reviewer` agent; this skill produces, it does not validate

---

## Output format

On success, produces:

- `docs/adr/{NNN}-{slug}.md` — the new (or superseded) ADR
- `docs/adr/README.md` — index updated; on supersede, both the new and the superseded rows are patched
- On supersede, the prior ADR's `Status` line is rewritten to `Superseded by ADR-{NEW}` (file otherwise untouched)

Reports the produced paths and the assigned ADR number to the conversation.

If the gate fails (Step 1.b) or the user declines the proposed decision (Step 2), do not write any file. Report:

```
ℹ️ No ADR created — {which criterion failed} ({one-line rationale}).
```

and exit. Silence is not an acceptable refusal; downstream callers parse this marker.

---

## The 3-criteria gate

ADRs are rare. Write one only when **all three** conditions hold:

1. **Genuinely complex** — the decision involves real trade-offs with no obvious right answer.
2. **Not obvious from context** — a future developer reading the code or the spec could not reasonably infer why this choice was made.
3. **Costly to reverse** — undoing the decision later would require significant rework across the codebase.

This block is the canonical source for the gate. `adr-reviewer` references it; do not restate it elsewhere in the kit without cross-linking back here.

---

## Execution Steps

### 1. Identify intent and validate the gate

a. **Resolve intent**. The user or another agent (e.g. `spec-writer`) will request one of:

- **Create** a new ADR
- **Supersede** an existing ADR
- **Refresh** the ADR index (rare standalone — usually a side-effect of create/supersede)

If the intent is ambiguous, use `AskUserQuestion`.

b. **Validate the 3-criteria gate** before writing anything. Use `AskUserQuestion` to confirm each of the three criteria holds (one yes/no per criterion). If any criterion fails, refuse per the Output format and exit. Do not proceed silently.

c. **Always confirm before writing**. Even when the intent is unambiguous (e.g. invoked by `spec-writer` after an `ADR-SUGGESTED`), surface the proposed decision to the user with `AskUserQuestion` and wait for explicit approval. Never auto-create.

---

### 2. Create a new ADR

1. List `docs/adr/` to determine the next available number (e.g. `003`). If `docs/adr/` does not exist, treat the next number as `001`.
2. Use `AskUserQuestion` to collect the **title** (short, imperative — e.g. "Use i64 for monetary amounts"). Then prompt the user for the **Context**, **Decision**, and **Consequences (Pros / Cons)** as a single free-form response — these are multi-sentence fields that don't fit a structured question.
3. Write `docs/adr/{NNN}-{slug}.md` using the template below.
4. Update the index (Step 4).

#### ADR file template

```markdown
# ADR {NNN} — {Decision Title}

**Date**: {YYYY-MM-DD}
**Status**: Accepted

## Context

{The problem, constraint, or forcing function. State what changed or what is new — why a decision is needed now.}

## Decision

{What was chosen and why this option over the alternatives. Name the alternatives considered.}

## Consequences

- **Pros**: {Concrete benefits — what becomes easier or possible.}
- **Cons**: {Concrete costs — what becomes harder or constrained.}
```

#### Worked example

For a decision "Use i64 for monetary amounts":

```markdown
# ADR 001 — Use i64 for monetary amounts

**Date**: 2026-05-09
**Status**: Accepted

## Context

The product handles currency end-to-end (frontend → IPC → backend → SQLite).
Floating-point representations (`f64`) introduce rounding drift visible to
users on aggregations. We need a single representation across all layers.

## Decision

Store and transport amounts as `i64` minor units (e.g. cents). Convert to a
display string only in the presenter layer. Considered: `f64` (rejected for
rounding), `Decimal` crate (rejected for SQLite friction).

## Consequences

- **Pros**: deterministic arithmetic, lossless SQLite storage, no FE/BE drift.
- **Cons**: every callsite must convert between minor units and display form;
  no native fractional currencies (e.g. some Middle-Eastern subdivisions).
```

---

### 3. Supersede an existing ADR

1. List `docs/adr/` and identify the ADR to supersede. If `docs/adr/` is empty or missing, inform the user there is nothing to supersede and offer to create a new ADR (Step 2) instead.
2. If the target ADR's status is already `Superseded by ADR-{X}`, the chain has been continued elsewhere. Refuse and point the user at ADR-{X} as the current decision.
3. Create the new ADR following Step 2 with `Status: Accepted — supersedes ADR-{OLD}`.
4. Patch the superseded ADR — change only its `Status` line to `Superseded by ADR-{NEW}`. Leave Context, Decision, Consequences untouched (history must remain readable).
5. Update the index (Step 4) — **both** rows must change: the new ADR's row, and the superseded ADR's row (its Status column now reads `Superseded by ADR-{NEW}`).

---

### 4. Update the ADR index

Maintain `docs/adr/README.md` as the navigable index of all ADRs.

1. Glob `docs/adr/*.md` excluding `README.md`.
2. Read each file's title (`# ADR {NNN} — ...`) and `Status` line.
3. Write `docs/adr/README.md`:

```markdown
# Architecture Decision Records

| ADR                          | Title   | Status                |
| ---------------------------- | ------- | --------------------- |
| [ADR-001](001-title-slug.md) | {Title} | Accepted              |
| [ADR-002](002-title-slug.md) | {Title} | Superseded by ADR-003 |
| [ADR-003](003-title-slug.md) | {Title} | Accepted              |
```

If `docs/adr/` does not exist, create it together with `README.md`.

---

## Critical Rules

1. **ADR numbers are permanent** — once assigned, never reused, even on supersede or removal.
2. **Never delete an ADR** — supersede it. Past decisions remain readable in their historical form.
3. **Status is one of three values** — `Accepted`, `Accepted — supersedes ADR-{NNN}`, `Superseded by ADR-{NNN}`. `Deprecated`, `Proposed`, `Rejected`, and free-form values are not allowed; tentative state belongs in the spec's `## Open Questions`.
4. **Always update the index** after creating or superseding — both rows on supersede.
5. **One decision per ADR** — split multiple decisions into separate ADRs; the reviewer enforces this.
6. **Decisions only, no implementation** — describe what was chosen and why, not how it is implemented in code.
7. **Validate the 3-criteria gate before writing** — refuse explicitly if any criterion fails.

---

## Notes

ADRs are deliberately rare. Most decisions belong in the spec (rules), in convention docs (coding standards), or are obvious from the code itself. The 3-criteria gate exists to keep `docs/adr/` valuable: a directory with five real ADRs is a reference; a directory with fifty mixed-quality ADRs is noise.

The interactive confirmation in Step 1.c is non-negotiable. `spec-writer` flags `ADR-SUGGESTED` candidates as part of its Open Questions output, but the user — not the agent chain — decides whether to elevate them. That gate prevents agents from filling `docs/adr/` with auto-generated decisions the user never ratified.

`adr-reviewer` is the paired validator. It runs after this skill produces or supersedes a file, and it cross-references back to the canonical 3-criteria gate above. Edit the gate language here only — the reviewer follows.
