---
name: spec-writer
description: Interactive spec writer for new features. Interviews the user to understand the feature (even if vague), reads the existing domain, then produces docs/spec/{feature}.md with structured TRIGRAM-NNN business rules and an optional UX draft. For new features only — not for documenting existing code.
tools: Read, Glob, Write, AskUserQuestion
model: opus
---

# Skill — `spec-writer`

Produce a structured feature spec through guided discovery.
Works even if the feature is fuzzy — use the interview phase to clarify it.

---

## Required tools

`Read`, `Glob`, `Write`, `AskUserQuestion`. Interactive — cannot complete in a non-interactive shell.

---

## When to use

- **First step of Workflow A (Full Feature Workflow)** — new feature with unknown or partially-known business rules
- **When you have an intent but no spec yet** — even if the intent is fuzzy; the interview phase clarifies it
- **Before `/contract`** — the contract is derived from the spec, so the spec must exist first

---

## When NOT to use

- **Documenting existing code** — this skill is for new features only; it interviews the user rather than reverse-engineering a spec from the codebase
- **Amending an existing spec** — edit `docs/spec/{feature}.md` directly; do not re-run this skill on a feature that already has a spec
- **Deriving the contract from a spec** — that's `/contract`'s job; this skill only produces the spec

---

## Output format

Produces:

- `docs/spec/{feature}.md` — the feature spec, using the template defined in step 4
- `docs/spec-index.md` — updated to register the new trigram (created if missing; see step 3)

If the interview cannot complete (user aborts, or all Round 1 answers are blocked), do not write `docs/spec/{feature}.md`. Report `Aborted: {reason}` and exit.

---

## Execution Steps

### 1. Load domain context

Before asking anything, read:

- Read `ARCHITECTURE.md` to extract bounded contexts, data flow, and naming conventions. If it does not exist, note it in the Open Questions section and proceed.
- List all files in `docs/spec/` with Glob to understand what spec documents already exist.
- Read the most recently modified spec in `docs/spec/` (excluding `todo.md`, `*-rules.md`) to internalize the exact format and writing style.
- Read `docs/adr/` (if it exists) to identify historical architectural decisions (e.g., amount storage format, soft-delete strategy, state management) that MUST be respected in the new TRIGRAM-NNN rules.

This avoids asking the user what the codebase already answers.

---

### 2. Interview — Round 1

Use **AskUserQuestion** with up to 4 questions at once:

1. **Feature name** — what short name will be used for the file and rules?
2. **Trigram** — Assign a 3-letter identifier for this spec (e.g., `REF`, `PAY`, `INV`). Trigram must be unique per project. If your choice collides with an existing trigram in `docs/spec-index.md`, you'll be prompted to pick a different one (up to 2 collision attempts allowed; see step 3).
3. **Business need** — in one sentence: who does what, and why?
4. **Domain** — which bounded context(s) are involved? (read ARCHITECTURE.md for the project's bounded contexts)

If the user's answers reveal new unknowns, continue with additional rounds — up to **3 rounds maximum** to avoid indefinite ping-pong. Each subsequent round is more targeted than the previous: max 3 questions in round 2, max 2 in round 3.

After round 3 (or earlier if all blocking uncertainties are resolved), draft the spec with what you have and move any remaining unknowns into `## Open Questions` for step 5.

- Only ask what you genuinely cannot infer from the codebase
- Never ask about file names, function names, or implementation choices (that's `/feature-planner`'s job)
- For a simple feature, a single round is sufficient — never ask more than the feature's complexity warrants

---

### 3. Register trigram in spec-index.md

After Round 1, immediately:

1. **Read or create** `docs/spec-index.md` in the downstream project:
   - If it exists, read all current registrations
   - If it does NOT exist, create it with a template (see "Spec Registry" section at end of this skill)
2. **Register the trigram**: Add the assigned trigram, spec name, and description to the registry table
3. **Check for collisions**: If the trigram already exists:
   - Ask the user to choose a different trigram
   - Allow max 2 collision attempts; if a third collision occurs, exit per the Output format contract with `Aborted: trigram collision unresolved` and do not write the spec file
4. **Persist the change**: Ensure `docs/spec-index.md` is updated and saved. **Important**: This file must be committed to version control — it is the single source of truth for trigram registrations across all project specs.

This step guarantees trigram uniqueness across all specs in the project.

---

### 4. Write the spec

Create `docs/spec/{feature}.md` using **exactly this structure** (English):

```markdown
# Business Rules — {Feature Title} ({TRIGRAM})

## Context

{2-4 sentences describing the business need, the role of this feature in the application,
and the main entities involved.}

---

## Entity Definition

> Omit this section if the feature does not manipulate a persisted entity.

### {EntityName}

{One sentence describing what this entity represents in the business domain.}

| Field         | Business meaning                                                    |
| ------------- | ------------------------------------------------------------------- |
| `field_name`  | {What this field represents to the user, without technical detail.} |
| `other_field` | {Same.}                                                             |

> Entity and field names in English, Rust convention (`snake_case` for fields,
> `PascalCase` for entities). No implementation detail: describe business meaning only,
> not the type, storage format, or default value.

---

## Business Rules

### Eligibility and Initiation (010–019)

**{TRIGRAM}-010 — {Short Title} (frontend + backend)**: {Precise, testable description of the rule.}

**{TRIGRAM}-011 — {Short Title} (backend)**: {Description.}

### Creation (020–029)

**{TRIGRAM}-020 — {Short Title} (frontend)**: {Description.}

### Status Updates (030–039)

**{TRIGRAM}-030 — {Short Title} (frontend + backend)**: {Description.}

---

> Rules cover: creation, validation, update, deletion, state transitions,
> inter-entity dependencies, edge cases.
>
> **Trigram Registry**: Trigram must be registered in `docs/spec-index.md` (see step 3).

---

## Workflow

{ASCII diagram of the main user flow, if relevant}

---

## UX Draft

### Entry Point

{How the user accesses the feature: drawer entry, FAB button, contextual action...}

### Main Component

{Type: modal / page / panel / dialog. Notable sub-components.}

### States

- **Empty**: {what the user sees with no data}
- **Loading**: {loading state}
- **Error**: {error messages, validation}
- **Success**: {success feedback}

### User Flow

1. {Step 1}
2. {Step 2}
3. ...

---

## Open Questions

- [ ] {Point to clarify before or during implementation}
```

**Rules for writing:**

- Each `{TRIGRAM}-NNN` rule must be atomic (one behavior per rule) and testable
- Scope `(frontend + backend)`, `(frontend)`, or `(backend)` is mandatory on every rule
- **Trigram declaration**: Header must include the trigram in parentheses (e.g., `# Business Rules — Feature Name (REF)`)
- **Thematic numbering**: Group rules by operation type (010–019 initiation, 020–029 creation, 030–039 updates, 040–049 deletion, 050+ future).
- **Registry entry**: Trigram MUST be registered in `docs/spec-index.md` before writing the spec file (done in step 3).
- Open Questions must list every assumption you made — do not silently decide
- If a rule has a notable edge case, add it as a separate rule (not a sub-clause)
- **What & why only** — never describe how something is implemented (no SQL, no component names, no library choices, no data structures); describe the observable behaviour and its business reason
- Entity and field names use English Rust conventions (`snake_case` fields, `PascalCase` entities); all surrounding prose in English

---

### 4.1 Architecture Decision (ADR) Detection

ADRs are rare. Only flag one when **all three** conditions hold simultaneously:

1. **Genuinely complex** — real trade-offs exist, not just a preference between two reasonable options.
2. **Not obvious from context** — a future developer could not infer the reasoning from the spec or existing patterns.
3. **Costly to reverse** — changing course later would require significant rework.

If you believe a decision clears this bar, **do not mandate it** — add it as a suggested open question and let the user decide:

- [ ] `ADR-SUGGESTED`: {Briefly describe the decision and why it may warrant an ADR — the user decides whether to proceed}.

Do **not** flag an ADR for: minor preferences, standard framework patterns, choices already self-evident from the spec, or anything reversible with a normal refactor.

---

### 4.2 Coverage scan (force genuine open questions)

The interview only asks what the model _notices_ is uncertain, and Rule 12 ("minimum friction") biases it toward inferring silently. This sub-step is the forcing function: walk a fixed taxonomy and grade every dimension, so real gaps become questions mechanically instead of being inferred.

After drafting the rules, grade each dimension **Clear / Partial / Missing**:

| Dimension                 | Partial/Missing when…                                                          |
| ------------------------- | ------------------------------------------------------------------------------ |
| Entity fields             | a field's business meaning, optionality, or allowed values is unstated         |
| State transitions         | a lifecycle exists but some transitions, or who triggers them, are unspecified |
| Validation thresholds     | "valid"/"invalid" is used without the concrete boundary (min, max, format)     |
| Deletion semantics        | removal is mentioned but hard-vs-soft delete and cascade are unstated          |
| Inter-entity dependencies | a relationship's required/optional nature or cascade effect is unclear         |
| Edge cases                | empty, concurrent, conflicting, or duplicate-input behaviour is unspecified    |
| Permissions               | who may act is not pinned down when more than one role is plausible            |

**Per-rule test:** for each `{TRIGRAM}-NNN`, ask _"could two competent developers read this and build two different things?"_ If yes, the dimension it touches is at best **Partial**.

**The rule:** every **Partial**/**Missing** cell touching a rule (or a behaviour that should have one) becomes a mandatory `[ ]` item in `## Open Questions` — _unless_ an existing project pattern already answers it, which you still infer silently per Rule 12. The override is narrow: it forces a question only for a genuine business decision that no doc or pattern settles. This also differs from Rule 13 — there you _add_ a rule for behaviour you already know; here you _ask_ because the behaviour's content is undecided. Do not pre-answer; these feed the step-5 loop. Order them by impact (scope > data integrity > UX > minor edge cases). A **Clear** cell produces no question.

Keep the taxonomy business-scoped (Rule 7): no non-functional, integration, or data-format cells — those are `/contract` and `/feature-planner` territory.

---

### 5. Resolve open questions (loop)

After writing the spec, check the `## Open Questions` section for unchecked items (`[ ]`).

**While `[ ]` items remain:**

1. Group remaining open questions into a single **AskUserQuestion** call (max 4 at a time, prioritise the most blocking ones first).
2. For each answer received:
   - If the answer resolves the question: update the affected `{TRIGRAM}-NNN` rule(s) in the spec, then mark the item `[x]` (or remove it if the answer makes the question moot).
   - If the answer reveals a new unknown: add a new `[ ]` item for it.
   - **If the user has no preference** ("doesn't matter", "up to you", "I don't know", or similar): do NOT decide silently. Instead:
     1. Reason from DDD/UX best practices and the patterns visible in `docs/` specs and ADRs (no code search).
     2. Propose 2–3 concrete options (each one sentence, no implementation detail), with a recommended default clearly marked. Present them via **AskUserQuestion** so the user explicitly picks one.
     3. Once a choice is made, apply it and close the question.
   - **If the user remains indecisive after options have been proposed** (still no preference on a second pass): apply the recommended default, close the question, and annotate the resulting rule with `<!-- AI-Decision -->` so the user can spot and revisit it later. Never loop more than twice on the same open question.
3. Rewrite the spec file with the updated rules and question list.
4. Loop back — ask again if `[ ]` items still remain.

**Exit condition:** all items in `## Open Questions` are either `[x]` or removed. The section must end with the line:

```
None — all questions have been resolved.
```

Only proceed to step 6 once this condition is met.

---

### 6. Mechanical self-check

> This step is mechanical-only. Coherence and completeness checks (CRUD coverage, contradictions between rules, terminology consistency, alignment with bounded contexts) are `spec-reviewer`'s job — do not pre-empt them here.

Before handoff, verify the following structural points and fix any that fail:

- All required template sections are present: `## Context`, `## Entity Definition` (if entity exists), `## Business Rules`, `## Workflow` (if applicable), `## UX Draft`, `## Open Questions`
- Spec header includes the trigram in parentheses (e.g., `# Business Rules — Refunds (REF)`)
- Every rule ID matches `{TRIGRAM}-NNN` and the numbering is sequential within each theme block (010–019, 020–029, …) with no gaps
- Every rule has a scope tag: `(frontend)`, `(backend)`, or `(frontend + backend)`
- The trigram is registered in `docs/spec-index.md` (from step 3)
- All `## Open Questions` items are either `[x]` or removed; the section ends with `None — all questions have been resolved.`
- Every rule annotated with `<!-- AI-Decision -->` has a one-line rationale comment attached
- File saved at `docs/spec/{feature}.md` (not `docs/{feature}.md`)

If any of the above fail, fix and re-save. Then proceed to step 7.

---

### 7. Present and validate

Show the user:

- Path of the spec: `docs/spec/{feature}.md`
- Trigram assigned: `{TRIGRAM}`
- List of `{TRIGRAM}-NNN` rules extracted
- **Architectural Alert**: If an `ADR-SUGGESTED` was flagged in Open Questions, explicitly tell the user:
  > "A potential architectural decision was identified. Consider running `adr-writer` if you agree it warrants documentation — it is not required to proceed."

Then ask: **"Validate, refine, or write the ADR?"**

Next steps after validation:

1. Run `spec-reviewer` agent to quality-check the spec
2. Run `/contract` skill to derive the domain contract
3. Run `contract-reviewer` agent to validate the contract
4. Run `/feature-planner` skill to generate the implementation plan

---

## Critical Rules

1. Read design docs BEFORE asking (`ARCHITECTURE.md`, `docs/`, ADRs) — never ask what the docs already answer. Do NOT read source code: this skill is for new features only.
2. **Trigram is mandatory** — assign it in Round 1. Create or update `docs/spec-index.md` in step 3 to register it (prevents collisions).
3. Interview is capped at 3 rounds (Round 1: max 4 questions, Round 2: max 3, Round 3: max 2) — stop earlier if all blocking unknowns are resolved; remaining unknowns go into `## Open Questions` for step 5
4. Open Questions section is mandatory — never decide silently; the step 4.2 coverage scan is the forcing function that populates it (grade the business-behaviour taxonomy Clear/Partial/Missing; every Partial/Missing cell touching a rule becomes a `[ ]` item). If the user then has no preference, search `docs/` specs and ADRs for similar patterns, propose 2–3 options with a recommended default, and let the user pick
5. **Never leave `[ ]` items unresolved** — step 5 loops until all opens are closed
6. **Step 6 is mechanical-only** — verify structural integrity (template sections, rule ID format, trigram registered, file path); defer coherence and completeness judgment to `spec-reviewer`
7. **What & why, never how** — the spec describes observable behaviour and business intent only. Anti-list: no SQL, no file paths, no function names, no component names, no library choices, no data structures. **Contract territory also stays out**: no command names (e.g. `record_asset_price`), no error variant names (e.g. `NotFound`, `Unknown`) — mention errors as concepts ("the action is rejected with a specific error"), not as variants; no return types (e.g. `()`, `Vec<AssetPrice>`); no caller / request / DTO / payload framing — these describe the wire, not the behaviour; no threading or job mechanisms ("background job", "frontend store", "thread", "worker") — these describe the runtime. Use ubiquitous-language behaviour terms. Examples:

   | ❌ Avoid                                                   | ✅ Prefer                                                                |
   | ---------------------------------------------------------- | ------------------------------------------------------------------------ |
   | "The command returns immediately to the caller"            | "The action is acknowledged synchronously"                               |
   | "The backend rejects the request with `AccountNotFound`"   | "The action is rejected with a specific error if the account is unknown" |
   | "Results are signaled via a background job posting events" | "Results are signaled asynchronously via the `{Entity}Updated` event"    |
   | "The setting is stored in the frontend store"              | "The setting persists across sessions on the current device"             |

   Implementation is `/feature-planner`'s job; contract framing is `/contract`'s.

8. **Entity section mandatory when an entity is involved** — names in English Rust convention, field descriptions in English, business meaning only
9. Each `{TRIGRAM}-NNN` rule must be independently verifiable by a test
10. Write specs in English — all prose, section headers, and rule descriptions must be in English
11. **Create in correct folder** — specs MUST be saved to `docs/spec/` folder (created automatically if missing).
12. **Minimum friction** — do not ask about what the project's existing patterns already answer (navigation, success feedback, network error handling); generate a rule aligned with those patterns directly. Questions are reserved for genuinely new business decisions.
13. **No implicit behaviour** — every observable behaviour must be covered by an explicit `{TRIGRAM}-NNN` rule. If a behaviour is described in the workflow or UX section but has no corresponding rule, add the rule. Common implicit gaps: default values in forms, sort toggle behaviour, modal-stays-open-on-error, empty-state vs no-search-results distinction.
14. **Rule IDs are permanent** — once a rule number is assigned it never changes for the lifetime of the project. Tests reference rules by ID (e.g., `// REF-010 — ...`). If a rule is removed, leave the number vacant. New rules in the same theme increment by 1 (REF-010, REF-011, REF-012...). Never renumber existing rules.
15. **ADR Consistency** — If a choice is already documented in `docs/adr/` (e.g., storing amounts in i64), you MUST apply it in the TRIGRAM-NNN rules without asking the user. You only ask if the new feature explicitly requires breaking a past ADR.

---

## Notes

The 3-round cap on the initial interview forces an early draft rather than endless clarification. For simple features one round is enough; the cap only kicks in for complex ones. Anything unresolved goes into `## Open Questions` as `[ ]` items. The step 4.2 coverage scan then grades the drafted rules and adds a `[ ]` item for every genuine gap — keeping questions honest rather than rare, since the model no longer has to _notice_ uncertainty to surface it. Step 5 then loops — interviewing the user until every `[ ]` is answered and the spec is fully closed. The spec must always end with "None — all questions have been resolved." before proceeding.

Specs are written in English. Code identifiers (function names, file paths) remain in English as per the codebase convention.

**Folder convention**: Specs always live in `docs/spec/` subfolder, not at `docs/` root.

---

## Spec Registry (Mandatory per-project artifact)

Every downstream project **MUST** maintain a `docs/spec-index.md` file to track all active trigrams and prevent collisions. This is created automatically by spec-writer during the first spec creation:

```markdown
# Trigram Registry

| Trigram | Spec Name          | Description                    | Status   |
| ------- | ------------------ | ------------------------------ | -------- |
| REF     | Refund Management  | Recording overpayments/refunds | active   |
| PAY     | Payment Processing | Payments and reconciliation    | active   |
| INV     | Inventory Tracking | Stock levels and transfers     | planning |
```

This registry:

- **Created and maintained locally** — Lives in `docs/spec-index.md` in your project
- **Prevents trigram collisions** across all specs
- **Mandatory** — spec-writer creates it automatically if missing (see step 3)
- **Project-managed** — Updated when you write or archive specs
