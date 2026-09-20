---
name: contract
description: Derives or updates a domain contract (docs/contracts/{domain}-contract.md) from a validated feature spec. Upsert-aware — adds commands without overwriting. Run after spec-reviewer approves, before contract-reviewer and `/feature-planner`. Not for validating an existing contract — use `contract-reviewer` instead.
tools: Read, Glob, Write, Edit, AskUserQuestion
model: opus
---

# Skill — `contract`

Produce or update a domain contract from a validated feature spec. The contract normalizes the backend ↔ frontend interface for a bounded context — commands, shared types, and error variants — so both sides can implement against the same shape.

---

## Required tools

`Read`, `Glob`, `Write`, `Edit`, `AskUserQuestion`. Interactive — cannot complete in a non-interactive shell.

---

## When to use

- **Third step of Workflow A** — after `spec-reviewer` green-lights the spec, before `contract-reviewer` validates the contract
- **When introducing a new bounded-context aggregate** — first-time contract for a domain
- **When extending an existing contract** — adding commands to an aggregate that already has a contract (this skill is upsert-aware and patches in place)

---

## When NOT to use

- **Validating an existing contract** — use the `contract-reviewer` agent; this skill produces, it does not validate
- **Producing the spec** — use `spec-writer` first; the contract is derived from the spec
- **Amending a single field** — edit `docs/contracts/{domain}-contract.md` directly; do not re-run the skill for trivial fixes
- **Generating the implementation plan** — `/feature-planner` consumes the contract; this skill stops at the contract

---

## Output format

Produces:

- `docs/contracts/{domain}-contract.md` — the domain contract file (created if missing; patched in place if existing, per step 5)
- A `## Changelog` entry appended inside that file (step 6)

If no frontend-callable commands are derived (step 3 — frontend-only or backend-internal-only feature), a duplicate command is detected across contracts (step 4), or the user rejects the diff (step 5), do not write or modify any file. Report `❌ Aborted — {reason}.` and exit.

---

## Execution Steps

### 1. Load spec

Ask for the spec path if not provided. Read `docs/spec/{feature}.md` in full.

If no spec path is given, list files in `docs/spec/` and ask the user which spec to use.

---

### 2. Identify domain

Extract the domain name from the spec's `## Context` section. The domain must name a
**bounded context (aggregate root)** — the `context/{domain}/` service that owns these
commands. Use-case folders (`use_cases/{domain}/`) are implementation details; their commands
belong in the aggregate contract they primarily mutate. The domain must NOT be named after a
frontend feature, page, or UI concern.

If it cannot be inferred, ask the user: "Which bounded context (aggregate) does this feature
belong to? (e.g. `user`, `portfolio`, `payment` — must match a `context/` folder)"

---

### 3. Extract contract data from spec

From the spec, derive:

**Commands** — one per **frontend-callable** backend operation. A rule describes a frontend-callable operation if it specifies an action triggered from the UI or an external caller. Internal-only logic (background jobs, startup tasks, scheduled workers, system monitors) is **not** a command — it has no frontend caller and no interface to normalize, so it does not appear in the contract.

If a backend rule is ambiguous about who triggers it (e.g., "transactions are reconciled on demand" — UI-triggered or system-internal?), use **AskUserQuestion** to ask before deciding. Never silently classify a borderline rule as internal-only — the omission is harder to recover from than a false-positive command.

For each command identify:

- `command`: `snake_case` name matching the spec rule's operation
- `args`: struct name and fields from the Entity Definition section
- `return`: the entity or value the command returns (from Entity Definition)
- `errors`: every failure condition described in the spec rule

**Shared Types** — every entity struct involved in args or return values. Use Rust field naming
(`snake_case` fields, `PascalCase` struct names). Describe business meaning only — no storage
types, no `Option<>`, no derives. Those are implementation details for `/feature-planner`.

**Events** — any named events implied by state-transition rules.

If no frontend-callable commands are derived — either because the spec is frontend-only (no `backend` rules) or because all backend rules describe internal-only logic (no frontend caller) — the feature has no backend ↔ frontend interface to normalize. Stop and report:

```
❌ Aborted — no frontend ↔ backend interface to normalize, no contract needed.
```

Skip steps 4–7. The user can re-run `/contract` later if frontend-callable backend operations are added to the spec.

---

### 4. Check for cross-contract command duplication

Before writing anything, glob all existing contracts and scan for command name conflicts:

- Run `Glob docs/contracts/*-contract.md` to collect every existing contract.
- For each contract found (excluding `{domain}-contract.md` itself), read it and collect its command names.
- If any command you are about to write already exists in another contract, **stop and report**:
  ```
  Command `{name}` already exists in docs/contracts/{other}-contract.md.
  Each command must belong to exactly one backend boundary. Resolve the overlap before proceeding.
  ```
- Do not write the contract until all conflicts are resolved.

### 5. Check for existing contract

Run `Glob docs/contracts/{domain}-contract.md`.

**If the file does not exist:**

- Compose the full contract (see format below)
- Show it to the user and ask: "Does this contract look correct? Any changes before I create it?"
- On approval, write `docs/contracts/{domain}-contract.md`

**If the file already exists:**

- Read the current content
- Identify what is new (commands not yet present) and what would be modified (changed args/return/errors)
- Present the diff to the user:
  ```
  New commands:    create_user, update_user
  Modified:        get_user — adding NotFound error variant
  Unchanged:       delete_user
  ```
- Ask: "Does this look correct? Any changes before I update the contract?"
- On approval, patch the file — append new commands, update modified rows, never remove existing commands silently

---

### 6. Write changelog entry

After writing or patching, append to the `## Changelog` section:

```
- {YYYY-MM-DD} — Added by `{feature-name}` spec: {comma-separated list of new/modified commands}
```

---

### 7. Confirm and hand off

Report using this shape:

```
✅ Contract written — docs/contracts/{domain}-contract.md
   Commands: create_user, update_user, delete_user
   Next: run `contract-reviewer` to validate, then `/feature-planner` for the implementation plan.
```

For an upsert (existing contract patched), prefix `Updated` instead of `Written` and list only the new/modified commands.

---

## Contract file format

```markdown
# Contract — {Domain}

> Domain: {domain}
> Last updated by: {spec-name}

## Commands

| Command           | Args                        | Return       | Errors                       |
| ----------------- | --------------------------- | ------------ | ---------------------------- |
| `snake_case_name` | `ArgStruct { field: Type }` | `ReturnType` | `ErrorVariant`, `OtherError` |

## Shared Types

\`\`\`rust
// Business-level struct — no derives, no Option, no storage detail
struct EntityName {
field_name: FieldType,
}
\`\`\`

## Events

| Event        | Payload       |
| ------------ | ------------- |
| `event_name` | `PayloadType` |

## Changelog

- {YYYY-MM-DD} — Added by `{spec-name}`: {command list}
```

---

## Critical Rules

1. Never silently overwrite existing commands — always diff and confirm with the user first
2. **One contract = one bounded context (aggregate root).** The domain must name a
   `context/{domain}/` bounded context, not a use case or module folder. Commands from
   `use_cases/` that primarily mutate one aggregate belong in that aggregate's contract. A
   frontend gateway may call commands from multiple contracts — that is expected. What is
   forbidden is the same command appearing in two contracts.
3. **No cross-contract command duplication.** If a command name already exists in another contract,
   stop and report before writing — do not proceed until the overlap is resolved.
4. **Features with no frontend ↔ backend interface get no contract.** The contract normalizes
   a frontend ↔ backend interface; a feature with no frontend-callable backend operations has
   nothing to normalize. This covers both frontend-only features (no backend rules) and
   backend-internal-only features (cron jobs, startup tasks, schedulers — backend rules with no
   frontend caller). Step 3 detects this and exits without writing a file.
5. Types use Rust naming conventions: `snake_case` fields, `PascalCase` structs
6. **Errors must be exhaustive.** Every failure path described in a spec rule must appear as a
   named error variant. A command whose only error variant is a generic catch-all
   (`DbError`, `InternalError`, `Unknown`) is **rejected** — name the specific failure modes
   from the spec (e.g., `NotFound`, `AlreadyExists`, `InvalidStatus`). Catch-all variants are
   acceptable only as a final fallback **alongside** named variants, never alone.
7. Do not invent commands not backed by a spec rule — traceability is mandatory
8. If the `docs/contracts/` directory does not exist, create it before writing
9. This skill produces the contract shape — `contract-reviewer` validates correctness
