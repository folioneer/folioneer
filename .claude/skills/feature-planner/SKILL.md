---
name: feature-planner
description: Translates a validated spec into an implementation plan (docs/plan/{feature}-plan.md) mapping TRIGRAM-NNN rules to DDD layers and the delivery workflow. Use after `spec-reviewer` and `contract-reviewer` approve. Output is gated by `plan-reviewer` before any test-writer runs. Not for spec or contract authoring — use `/spec-writer` or `/contract` instead.
tools: Read, Write, Grep, Glob, Bash, AskUserQuestion
model: opus
---

# Skill — `feature-planner`

Invocation: `/feature-planner`

Maps TRIGRAM-NNN rules from a validated spec onto concrete DDD layer tasks, under ADR constraints, producing an executable plan that downstream test-writers and implementers can follow without re-reading the spec.

---

## Required tools

`Read`, `Write`, `Grep`, `Glob`, `Bash`, `AskUserQuestion`. `Bash` invokes `scripts/plan-context.py` (Step 1). Interactive — Step 6 asks the user how to slice the merge into PRs.

---

## When to use

- **Phase 1 of Workflow A** — after `spec-reviewer` and `contract-reviewer` both green-light their artifacts
- **When the spec has at least one TRIGRAM-NNN rule** — a spec with no rules is incomplete; redirect the user to `/spec-writer` first
- **When introducing a new feature** — produces the first plan for `docs/plan/{feature}-plan.md`

---

## When NOT to use

- **Authoring the spec** — use `/spec-writer`; the plan is derived from a validated spec
- **Authoring the contract** — use `/contract`; the plan consumes the contract
- **Validating the plan** — use the `plan-reviewer` agent
- **Implementing the plan** — use `test-writer-*` agents to write failing tests first; the main agent then implements against them

---

## Critical Rules

1. **Path Verification**: Every file path must be verified with `Glob` before being included — never invent paths.
2. **Synchronization**: Include `just generate-types` as a mandatory step between Backend and Frontend tasks — Specta regenerates `src/bindings.ts` from the Tauri command surface.
3. **No Code Implementation**: Your output is a plan describing _what_ to do and _where_, not the actual code.
4. **Task Tracking**: Ensure the main agent can progressively update the checkboxes in this file during the implementation phase.
5. **Commit Checkpoints**: Every plan must include at least one commit checkpoint per thematic phase (backend, frontend, E2E tests, tests & docs). Each checkpoint provides only a suggested conventional commit title; the commit hook validates the format and the agent writes the final message.
6. **Minimal implementation**: Implementation tasks must state "implement only what makes failing tests pass — no defensive code, no anticipation of future rules." `test-writer-backend` and `test-writer-frontend` define the scope; the implementation must not exceed it.
7. **Next gate**: After writing the plan, tell the user that `plan-reviewer` is the mandatory next step before any test-writer subagent runs. Do not invoke it yourself; the orchestrating agent runs it.

---

## Execution Steps

### 1. Collect plan context

If the feature name or spec path is ambiguous, ask the user via `AskUserQuestion` before proceeding.

Run `python3 scripts/plan-context.py docs/spec/{feature}.md` and parse the JSON output as `PLAN_CONTEXT`. It contains:

- `spec`: `path`, `found`, `trigram`, `registered`, `rules` (list of `{id, scope, description}`)
- `layout`: `architecture_present`, `backend_root`, `frontend_root`, `fallbacks_used`
- `conventions`: presence map (`architecture_md`, `backend_rules`, `frontend_rules`, `ddd_reference`, `error_model`, `i18n_rules`, `test_convention`, `frontend_visual_proof`, `e2e_rules`)
- `adrs`: list of `{path, title, status}`

If `PLAN_CONTEXT.spec.found` is `false` or `PLAN_CONTEXT.spec.rules` is empty, report and ask the user to complete via `/spec-writer` before proceeding.

If `PLAN_CONTEXT.spec.trigram` is `null` but `PLAN_CONTEXT.spec.rules` is non-empty, the spec contains rules from more than one TRIGRAM. Ask the user via `AskUserQuestion` which trigram owns this feature before proceeding — do not silently pick one.

If `PLAN_CONTEXT.spec.registered` is `false` (and a trigram was resolved), flag the missing `docs/spec-index.md` entry (spec-writer Step 2.5 should have handled it).

From `PLAN_CONTEXT.spec.rules`, identify entities, UI components, and cross-context dependencies. From `PLAN_CONTEXT.adrs`, identify which ADRs constrain this feature (judgment call based on rule content + ADR titles) — list them in the **Setup** TaskList for the implementer to re-read before coding. If `PLAN_CONTEXT.adrs` is empty (fresh project without `docs/adr/`), omit the ADR line from Setup.

### 2. Architectural Contextualization

From `PLAN_CONTEXT.conventions`, determine the per-feature scope subset to read (and to list in the **Setup** TaskList):

- Always: `ARCHITECTURE.md` (if present), `docs/test_convention.md`
- BE-relevant (any rule with `scope: backend` or `frontend + backend`): + `docs/backend-rules.md`, `docs/ddd-reference.md`, `docs/error-model.md`
- FE-relevant (any rule with `scope: frontend` or `frontend + backend`): + `docs/frontend-rules.md`, `docs/i18n-rules.md`, `docs/frontend-visual-proof.md`

Read the chosen docs in full for compliance during Step 4 mapping (skip silently if a doc shows `false` in `PLAN_CONTEXT.conventions`).

**Contract**: derive the domain name from the spec's Context section; `Read` `docs/contracts/{domain}-contract.md`. If present, treat as mandatory — its commands anchor the `test-writer-*` tasks in the plan. Frontend-only or backend-internal-only features have no contract per `/contract`; in that case, proceed without one.

### 3. Path Verification

`PLAN_CONTEXT.layout` has `backend_root`, `frontend_root`, `architecture_present`, and `fallbacks_used`. If `architecture_present` is `false`, the roots come from fallback search — note this assumption in the plan.

Verify any new path the plan will reference (beyond the discovered roots) with `Glob` or `Grep` before including it.

### 4. Mapping & Dependency Graph

For each TRIGRAM-NNN rule, identify concrete tasks:

- Determine if it requires a creation or a modification.
- Map which layer(s) are affected.
- **ADR Application**: Explicitly mention ADR constraints in the tasks (e.g., "Implement amount using i64 as per ADR-001").
- Define dependencies (e.g., Backend logic -> `just generate-types` -> Frontend gateway).
- **Commit phases**: identify thematic boundaries where a commit is appropriate. Suggest a conventional commit title for each (e.g., `feat(asset): implement pricing backend`).
- **Schema changes**: identify rules that imply a database schema change (new entity, new field, new status column, new FK). For each, note the expected migration filename (`{timestamp}_create_{table}.sql` or `{timestamp}_add_{column}_to_{table}.sql`) and infer the columns from the domain rules. Flag that `just prepare-sqlx` must be run after migrating.

### 5. Modified-function coverage

For each rule scoped to `frontend` or `frontend + backend` that **modifies an existing function** (not a new file/creation):

- Mark the rule `[unit-test-needed]` in the Rules Coverage table.
- Collect the affected functions as a `modified_functions` list `[<file>:<function>]` (e.g. `[useEditFoo.ts:recomputeUnitPrice]`), attached to the `test-writer-frontend` checkpoint in the Workflow TaskList so the test-writer receives it alongside the contract.

Example trigger: rule REF-040 says _"when discount changes, recompute `unit_price`"_ — this modifies `useEditRefund.ts:applyDiscount`, so REF-040 gets `[unit-test-needed]` and `useEditRefund.ts:applyDiscount` joins the `modified_functions` list. These rules have no contract entry and would otherwise receive no test coverage.

### 6. PR Plan

From the tasks identified in Step 4, estimate the change size per layer. Heuristic: count target file paths from Step 4's Mapping, then multiply by ~30 LOC per new file (less for modifications to existing files). The estimate is advisory — it pre-selects the recommended slicing but the user picks. Then ask via `AskUserQuestion`:

- **1 PR** — single merge covering all layers. Recommended for small or tightly-coupled features (rename, contract reshape, migration that demands both layers in lockstep).
- **2 PRs** — backend (spec + migration + Rust + bindings) merges first; frontend + E2E + closure follow on a branch rebased off `main` after the BE PR merges.
- **3 PRs** — backend; then frontend (gateway / hooks / components / i18n); then E2E + ARCHITECTURE/todo/spec-checker closure. Each PR branches off the prior one once merged.

Pre-select based on the estimate: if **either layer exceeds ~20 files OR ~500 LOC**, recommend 2 PRs (or 3 if E2E adds significant volume); otherwise recommend 1 PR. The estimate is advisory — the user picks; coupling judgment trumps the threshold.

Surface the estimate explicitly in the question (e.g. "BE: ~14 files / ~380 LOC; FE: ~9 files / ~210 LOC"). Capture the answer verbatim in the deliverable's **PR Plan** section.

---

## Output format

You MUST generate and **WRITE** `docs/plan/{feature}-plan.md` with the following sections:

### 1. Workflow TaskList (Derived from CLAUDE.md)

A synthetic checklist for mandatory quality and process steps, grouped by phase. Skip an entire phase if its top-level condition does not apply.

**Setup** _(planner fills in exact paths per this feature — Steps 1 + 2 produce the lists)_

- [ ] 📖 Read spec: `docs/spec/{feature}.md`
- [ ] 📖 Read contract: `docs/contracts/{domain}-contract.md` — _omit if frontend-only or backend-internal-only (no contract)_
- [ ] 📖 Read constraining ADRs: list per this feature (e.g. `docs/adr/001-currency-types.md`) — _omit if none apply_
- [ ] 📖 Read conventions: `ARCHITECTURE.md` + per-scope subset (e.g. `docs/backend-rules.md`, `docs/error-model.md`, `docs/ddd-reference.md` for BE; `docs/frontend-rules.md`, `docs/i18n-rules.md` for FE; `docs/test_convention.md` always)

**Backend phase** _(skip if no backend rules)_

- [ ] 🗄️ Database Migration (`just migrate` + `just prepare-sqlx`) — if schema changes required
- [ ] ✍️ Backend test stubs (`test-writer-backend` — all stubs written, red confirmed)
- [ ] 🏗️ Backend Implementation (minimal — make failing tests pass, green confirmed)
- [ ] 🔍 Backend Review (`reviewer-backend` + `reviewer-arch` + `reviewer-sql` if migrations — all in parallel → `/review-triage` → apply Follow-ups)
- [ ] 🔗 Type Synchronization (`just generate-types`)
- [ ] 🔧 Run `npx tsc --noEmit` → fix TS errors from new bindings only (no UI work)
- [ ] 🧹 `just format` (rustfmt + clippy --fix)
- [ ] 💾 Commit: backend layer (suggested title from plan)
- [ ] 🔀 Open the PR — if the PR Plan slices BE into its own PR; otherwise continue. After merge, branch the next phase off updated `main`.

**Frontend phase** _(skip if no frontend rules)_

- [ ] ✍️ Frontend test stubs (`test-writer-frontend` — all stubs written, red confirmed; pass `modified_functions` list if any `[unit-test-needed]` rules are present)
- [ ] 💻 Frontend Implementation (minimal — make failing tests pass, green confirmed)
- [ ] 📸 Visual proof (`/visual-proof` — capture final state; stage screenshots before commit) — if .tsx/.css changed
- [ ] 🔍 Frontend Review (`reviewer-frontend` → `/review-triage` → apply Follow-ups)
- [ ] 🧹 `just format`
- [ ] 💾 Commit: frontend layer (suggested title from plan)
- [ ] 🔀 Open the PR — if the PR Plan slices FE into its own PR; otherwise continue. After merge, branch the next phase off updated `main`.

**Closure** _(always)_

- [ ] ✍️ E2E scenarios (`test-writer-e2e` — produces pyramid-friendly scenarios; run `/setup-e2e` first if not done) — if E2E coverage applies
- [ ] ▶️ Run E2E suite (`npm run test:e2e` → green confirmed; main agent triages any failure) — if E2E coverage applies
- [ ] 🔍 Cross-cutting Review (`reviewer-e2e` if E2E test files modified + `reviewer-infra` if config/script/hook/workflow files changed + `reviewer-security` if Tauri command/capability/sensitive file modified — all in parallel → `/review-triage` → apply Follow-ups)
- [ ] 📚 Documentation Update (`docs/todo.md` — close shipped entries; `ARCHITECTURE.md` only if a new module/path or layer pattern was introduced)
- [ ] ✅ Spec check (`spec-checker`) [HARD GATE — halt on any uncovered rule or command]
- [ ] 🧹 `just format`
- [ ] 💾 Commit: closure (suggested title from plan)
- [ ] 🔀 Open the final PR per the PR Plan; merge on green with `just merge`

### 2. Detailed Implementation Plan

A granular breakdown by architectural layer:

- **Migrations** (if any): List each migration file with its suggested filename, inferred columns, and a reminder to run `just migrate` then `just prepare-sqlx` before writing backend code. Omit this section if no schema changes are required.
- **Backend**: Exact file paths, struct names, factory methods (follow project conventions from `docs/backend-rules.md`), service methods, and command handlers.
- **Frontend**: Exact file paths, gateway methods, custom hooks, and frontend components.
- **Rules Coverage**: see table below.

#### Rules Coverage

A table mapping every TRIGRAM-NNN rule to its corresponding implementation task. Example rows:

| Rule    | Layer    | Task                                | Notes                |
| ------- | -------- | ----------------------------------- | -------------------- |
| REF-010 | backend  | `RefundService::record_overpayment` | ADR-001 (i64)        |
| REF-040 | frontend | `useEditRefund.ts:applyDiscount`    | `[unit-test-needed]` |

### 3. PR Plan

Captures the answer from Step 6. Format:

- **Strategy**: `1 PR` | `2 PRs` | `3 PRs`
- **Estimate**: per-layer file count and LOC used to pre-select.
- **PR list** — for each planned PR:
  - **Title** (conventional commit format, e.g. `feat(asset): pricing backend`)
  - **Scope**: which layer(s) and which Workflow TaskList checkpoints terminate it
  - **Dependency**: what must be merged first (e.g. "rebase off main after PR #1 merges")
  - **Branch suffix** (suggestion): `feat/{name}-be`, `feat/{name}-fe`, `feat/{name}-e2e` for multi-PR strategies; `feat/{name}` for single-PR

The agent reads this section to decide where the PR boundaries fall.
