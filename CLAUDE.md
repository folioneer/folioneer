# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> Full architecture reference: [ARCHITECTURE.md](ARCHITECTURE.md) · How work moves: [docs/workflow-c.md](docs/workflow-c.md)

## 🔧 First-time Setup

After cloning, activate the git hooks:

```bash
git config core.hooksPath .githooks
```

They block direct commits to `main`, validate the conventional-commit format, reject `Co-Authored-By` lines, and run the fast checks scoped to what the commit or push touches (`scripts/changed-scope.sh`: Markdown-only gets Prettier, frontend-only skips cargo, backend-only skips the web toolchain). Tests, coverage, E2E and the build are CI's job on the pull request.

## 🧭 Who decides what

Four human touchpoints, everything else is the agent's job behind the harness (`docs/workflow-c.md` § 1):

- The **human** writes `docs/todo.md` (user value, done-when) and its **Next** queue, validates a **design** before anything the user sees changes, and cuts **releases**.
- The **agent** owns `docs/techdebt.md`, does the task end to end and merges on green. No pull request is validated by a human.
- The **harness** (`just harness` locally, required checks in CI) proves the code: lint, types, build, both test suites, coverage floors, architecture rules, E2E on the real app.

In a headless run, questions that only the human can answer are written into the entry's `**Open questions:**` line, never asked. In a chat run (`/next-todo #NNN`), the open questions are asked together, once, before anything starts, and a design is validated by a yes in the conversation; the answers are written into the entry either way. In a chat conversation with the human (designing, planning, discussing), ask as you would with a colleague: state assumptions, name what is unclear.

## ⚠️ Core Rules

1. **Authority follows the entry.** While running a queued entry (`/next-todo`), a `TD-NNN` the human queued, or a phase the human said "go" to, the agent branches, commits, pushes, opens the PR and runs `just merge` on green without asking. In an open-ended chat, ask once for the task, not per step. Always: never push to `main` directly, never force-push, never bypass a hook, never cut a release, never touch the live portfolio database (`~/.local/share/com.folioneer.desktop/`, and `~/.local/share/com.phileggel.vault-compass/` which holds it until the carry-over and stays as its backup, are read-only reference data).
2. **Always use `just`**: never run native commands (`cargo build`, `npm install`, `sqlx migrate`) when a recipe exists in the `justfile`.
3. **Every change goes through the harness**: branch → PR → every check green → `just merge`. Docs-only changes too. The entry's Done when is the plan; there is no separate plan-approval gate. Anything the user sees changing goes through the design gate first (`/design-proposal`).

## 🎯 Per-task Discipline

Each task ships under these constraints (in priority order):

1. **Surgical** — touch only the file set the task requires. Refuse "while I'm here" expansions outside that set. Every PR tells one story.
2. **Gold standard for new code; bit-by-bit for existing** — apply gold standards to new code (backend layout per `docs/backend-rules.md` B0/B37–B43, FE layout per `docs/frontend-rules.md` F0/F26–F28, typed error model per `docs/error-model.md`). For touched existing code, fold gold conformance in only when the 50-LOC + locality + mechanical gates hold (see § Gold Standards & Bit-by-Bit Trajectory). When in doubt, defer.
3. **Boyscout** — small mechanical fixes inside the files you're already editing (dead code, misleading test names, typos) ship in the same PR. Stay inside the touched file set; don't go on adjacent quests.
   - **Never maintain known dead code.** Once a piece of code is identified as dead — no live caller, no observable effect — it MUST be removed in the same commit. Surface the audit (live vs dead table) in the PR body and delete.
   - **No transition comments** — no tombstones like `// X was migrated to Y in PR N`. Git history carries the trail. Doc comments describe what the code IS, not what it USED TO BE.
4. **Coverage when a real gap surfaces** — if a task lands you next to an untested branch / unverified invariant / missing translation assertion in the touched module, add a focused test. The floors in `coverage-gates.json` are a ratchet: raise them in the change that lifts coverage, never lower them.
5. **Challenge reviewer returns** — every reviewer finding is graded on `/review-triage`'s axes and the outcome recorded in the PR body (`docs/workflow-c.md` § 7): (a) **actionable in scope** → fix now; (b) **actionable but bigger** → `TD-NNN` in `docs/techdebt.md`; (c) **false positive** → one-off: inline `// <reviewer> FP: <reason> — see PR #NN`; pattern: edit the reviewer prompt in the same PR. A `[DECISION]` critical becomes an open question on the entry. "Pre-existing" alone never decides the grade; it only routes through the boyscout test.
6. **PR size target ≤1000 LOC** — insertions + deletions. Not a hard cap; split when a PR crosses it OR tells two stories. The "two stories" check overrides the number.

## 🧾 Opening and closing a piece of work

**Opening brief** — four lines before the first edit, so the shape is fixed while nothing is at stake and the human can stop it being the wrong shape. In a chat run it is the first message; in a headless run it opens the PR body.

    **Task**     — the entry (`#NNN` / `TD-NNN`) or the request, in one line
    **Scope**    — the commit type and the layers (backend / frontend / E2E / docs / CI)
    **Design**   — none, validated, or needed (then the mocks come before anything else)
    **Touching** — the paths, so the reviewer lanes are known before the diff exists

**Closing brief** — the last message of a piece of work (and the PR body's first lines). Two parts, in this order, nothing else; what is still owed goes in `docs/techdebt.md` (or, before the merge, an open question on the entry), not here. A mistake is worth a sentence only where it left something undone.

1. **What changed for whoever reads this next.** For `feat` / `fix`: what a **user** notices, or "nothing — internal". For everything else: what can now be **done or trusted** that could not be before. "The backend job answers in 8 minutes" is an answer; "switched to llvm-cov" names the artefact, not what it buys.
2. **What the project accumulated.** For logic work: the tests added and the coverage or golden figure that moved, or "none". For harness work: the guarantee and **what it is worth** — a check that cannot fail has bought nothing, so say how you know it can, which usually means having watched it go red.

A part with nothing in it gets one line.

---

## 🔄 Workflows

**Workflow C** (`docs/workflow-c.md`) is the workflow: `/next-todo` runs the first ready entry of `docs/todo.md` § Next end to end — branch, design gate, acceptance tests first, implement, `just harness`, reviewers with the triage policy, PR, merge on green, closure. One entry per invocation.

Key skills: `/next-todo` (run an entry), `/design-proposal NNN` (mocks for the human to validate), `/visual-proof` (screenshots of changed components), `/review-triage` (grading axes for reviewer findings), `/techdebt` (entry format), `/spec-writer` (business rules for a feature the human is designing in chat), `/contract` (the wire record when commands change), `/adr-writer` (architecture decisions), `/dep-audit` (dependency CVEs), `/prune` (dead-code audit), `/session-reflect` (end-of-session rule audit).
Key recipes: `just harness` (the merge gate, locally), `just arch-check` (architecture rules; `--write-allowlist` only lowers the frozen debt), `just coverage-gate` (floors; run `coverage-fe` / `coverage-be` first), `just check` (lint/format), `just check-full` (tests + build + lint), `just format` (auto-fix), `just generate-types` (regenerate Specta bindings), `just merge` (rebase, refuse unless every check on the pull request is green, fast-forward, push, delete branch), `just release` (full validation → semver bump → CHANGELOG → commit + tag + push).
Key agents: the reviewers matched to the diff, locally until no 🔴 remains and in CI on every push (`docs/workflow-c.md` § 7); `reviewer-security` also before every release; `spec-checker` before closing an entry that carries spec rules; `spec-reviewer` / `contract-reviewer` / `adr-reviewer` when those documents change.

**Only prescribed agents.** Launch exclusively the agents this file, a skill or `docs/workflow-c.md` names. Implementation and review fixes are done by the main agent directly — never a general-purpose "implementer" or "fix" agent; a fresh agent lacks the session's settled decisions and drifts.

### Mandatory pre-read by task type

Before implementing, read the relevant convention docs:

- **Backend changes** — `docs/backend-rules.md` + `docs/ddd-reference.md` (especially when touching the error model — see [`docs/error-model.md`](docs/error-model.md)). Project-specific idiomatic patterns (row mapping, orchestrator shape) live in [`docs/backend-patterns.md`](docs/backend-patterns.md).
- **Frontend changes** — `docs/frontend-rules.md` + `docs/i18n-rules.md` + `docs/frontend-visual-proof.md`. Run `/visual-proof` after implementation to capture all states in light + dark mode.
- **E2E changes** — `docs/e2e-rules.md`.
- **Any test work** (unit / integration / E2E, BE or FE) — `docs/test_convention.md`.

The convention docs are project files: when a rule changes, edit the doc in the same PR. Intentional deviations from textbook DDD live in `docs/ddd-divergences.md`.

### After completion — update the source doc

When work resolves a todo entry, an open question, or a tech-debt observation, update the source doc in the same PR. `docs/techdebt.md` for non-actionable smells, `/spec-writer` + `spec-reviewer` for new business rules, `/contract` + `contract-reviewer` for the matching contract, `/adr-writer` + `adr-reviewer` for architectural decisions. When an empirical failure mode bites (especially external tooling), append an `L-NNN` entry to [`docs/lessons.md`](docs/lessons.md) — symptom / trigger / root cause / mitigation, one screen. Update `ARCHITECTURE.md` if new files/modules were added.

### Task tracking (within a conversation)

For every implementation task, use `TaskCreate` / `TaskUpdate`: create tasks before implementing anything non-trivial (>1 file or >1 step); mark each `in_progress` when starting, `completed` immediately when done.

### PR strategy — split per layer for non-trivial features

For features that touch both backend and frontend, **default to one PR per layer** when either layer exceeds ~20 changed files or ~500 LOC. Below that threshold a single PR is fine.

When splitting, the order is **BE → FE → E2E**:

1. **Spec / contract / migration / backend domain + service + api + bindings** — first PR. Mergeable on its own (FE doesn't yet consume the new types but TS bindings are present and unused, no runtime impact).
2. **Frontend gateway / hooks / presenter / components / i18n** — second PR, branched off the merged BE branch. Reviewable against a stable backend.
3. **E2E tests + ARCHITECTURE / todo / spec-checker closure** — third PR. Run `reviewer-e2e` on the E2E test files.

Why: a 60-file mixed-layer PR sprawls across concerns; per-layer PRs keep each diff tight, let CI sign off independently, and let the backend ship before the frontend reacts to the bindings.

---

## 🛠 Commands

- Dev: `./scripts/start-app.sh`
- Loop: `just next-todo` — one ready entry of `docs/todo.md` § Next, headless (`docs/workflow-c.md` § 9)
- Tests: `just test` (frontend) | `just test-rust` (backend) | `just test-unit` (both)
- E2E tests: `just test-e2e` (local) | `just test-e2e-headless` (Linux headless). CI runs the suite on every pull request and every push to `main`; the local run on this machine is known-broken (`docs/lessons.md` L-011), so CI is the gate.
- Security audit: `/security-review` (IPC, capabilities, SQL injection, hardcoded secrets) — Claude Code built-in, run before release alongside `/dep-audit`
- Release sequence (human): `/dep-audit` → `just release [--preview] [--dry-run] [--version X.Y.Z] [-y]` (use `--preview` first to see the next computed version without side effects). `just release` runs `check-full`; every merged PR has already passed E2E. The Release workflow builds Windows first, then Linux, and leaves the GitHub release as a **draft** — finish with `gh release edit vX.Y.Z --draft=false` once both jobs have attached their assets; the in-app updater only sees published releases. On Linux the AppImage is the self-updating channel; the `.deb` is manual-install only.

## 📖 Ubiquitous Language

`docs/ubiquitous-language.md` is the authoritative dictionary of domain terms.

- New code MUST use confirmed UL terms in identifiers, comments, and log messages.
- Do not extend usage of a discrepant term — fix it or flag it before adding more callsites.
- When spawning reviewer, spec-writer, or feature-planner agents, include the UL doc in the prompt so they can check term consistency.

## 🏗 Architecture Summary

Tauri 2 app (React 19 + Rust) using Domain-Driven Design.

**Backend (`src-tauri/src/`)**:

- `core/specta_builder.rs` — Tauri command registry (DO NOT add commands elsewhere)
- `context/{domain}/` — Bounded contexts (self-contained, no cross-context imports — `scripts/arch-check.py` A3 enforces it):
  - `account/`, `asset/` (older layout: `domain/`, `repository/`, `service.rs`, `api.rs`), `currency/`, `sync/` (gold layout: `domain/`, `application/`, `infrastructure/`, `api.rs`)
- `use_cases/` — cross-cutting application use cases, one folder per use case with an `orchestrator.rs` and an `api.rs`
- `shared/` — kernel shared across contexts (domain records, change capture)

**Frontend (`src/`)**:

- `bindings.ts` — Auto-generated from Rust via Specta (DO NOT EDIT)
- `features/{domain}/` — Feature modules (gold layout: `assets/`):
  - `gateway.ts` at root — only file allowed to call `commands.*` (arch-check A1)
  - Sub-feature subdirectories with colocated component + hook + test
  - `shared/presenter.ts` — domain → UI transformations; `shared/validate*.ts` — validation
- `features/shell/` — the composition root: layout chrome and the modal mounts that host other features' modals (the one place sibling-feature imports are allowed, arch-check A2)

**Data Flow**: Component → Hook → Gateway → Tauri Command → Rust Service → Repository

## 🥇 Gold Standards & Bit-by-Bit Trajectory

The project has three evolving "gold" targets the codebase moves toward **bit by bit** over time. Future sessions follow them for **new code** and for **small surgical updates** to existing code, but **never trigger a big-bang refactor** to make existing code conformant.

### The three golds

1. **Backend layout gold** — rules `B0`, `B37`–`B43` in `docs/backend-rules.md`. New code under `shared/` (not `core/`), `context/{bc}/{application,domain,infrastructure}/` symmetric trio, `infrastructure/` (not `repository/`).
2. **Frontend layout gold** — rule `F0` in `docs/frontend-rules.md` defines the canonical `src/` tree (`features/`, `shell/`, `ui/`, `infra/` + framework exceptions `assets/` / `styles/` / `public/`); rule `F28` is the include/reject discipline that supports F0. New code follows F0; the existing `src/lib/` → `src/infra/` rename and the cross-feature import reframe are bit-by-bit migration targets whose remaining crossings are frozen in `arch-allowlist.json`. **Project divergence**: `App.tsx` stays at `src/` root (Vite/CRA convention) rather than moving to `src/shell/`; see [`docs/ddd-divergences.md`](docs/ddd-divergences.md).
3. **Error-model gold** — landed. Canonical reference: [`docs/error-model.md`](docs/error-model.md). Per-BC `*ApplicationError::DatabaseError`; shared `InfrastructureError` was removed from the FE wire surface; application layer translates raw infra errors and logs server-side via `tracing::error!`. See also [`docs/ddd-divergences.md`](docs/ddd-divergences.md) for the intentional deviations from textbook DDD.

### Bit-by-bit update rule

Apply gold to **new code** (new files, new commands, new error variants, new features). For **existing code that touches a gold-standard area** during a task, fold gold conformance into the current task ONLY when ALL three hold:

- **Size**: ≤50 LOC of conformance changes (a checkpoint number, not a magic threshold — see "two stories" check below).
- **Locality**: changes stay within the natural file set the task already touches. Don't pull unrelated files into the diff just to gold-conform them.
- **Mechanical**: rename, import update, signature swap, type substitution. Any fresh **design judgement** ("which layer does this belong in?", "what should this variant be named?") triggers defer even if the line count is small — that's a design call that deserves its own entry.

If any of the three fails, **DO NOT refactor** — match the current project standard in the touched area and continue. The bigger gold migrations are tracked in `docs/techdebt.md` and ratcheted in their own entries when the human queues them.

**The "two stories" sanity check** (overrides the LOC number when in tension): would a reviewer say this PR is telling **one story** (the feature/fix) or **two stories** (the feature/fix + a layout migration)? If two, the gold conformance IS the second story — defer it.

**Consistency is not the goal**: if a touched area is currently using the OLD project standard and the surrounding code is OLD, KEEP IT OLD when conformance would breach the 50-LOC threshold. A mixed-standard codebase is acceptable during the bit-by-bit migration; what is NOT acceptable is a partial mid-flight refactor that leaves neither standard intact.

## 📏 Standards

- **Nothing about credentials in public text.** PR bodies, PR comments and commit messages never mention secrets, tokens, keys or how they are handled; that belongs in the workflow file's own comments and in `docs/workflow-c.md`.
- **Concise by default.** Everything the agent writes for a reader — review reports, PR bodies, techdebt entries, docs, commit bodies, chat — states each fact once, in the fewest words that keep it verifiable. A finding is one line: location, claim, fix. A PR body says what changed and what proves it, in under 20 lines; the triage table lists only findings that changed something. No restating the diff, no narrative of how something was found, no alternatives the reader did not ask for, no paragraph where a line does. A sentence that a reader cannot act on is cut.
- **Commits**: Conventional commits (`feat:`, `fix:`, etc.). **Titles target the user, not the developer** — `feat`/`fix` titles are copied into `CHANGELOG.md` (titles only; bodies stay in git history) and shown in-app by the What's-new dialog. Describe the user-visible outcome in plain words: no layer tags (`(FE)`/`(BE)`/"frontend"/"backend"), no rule IDs or trigrams (`SPL-040`), no code identifiers, no abbreviations. The technical why stays in the body (≤2 lines). Titles are ≤72 characters (hook-enforced).
- **One changelog line per user-visible change**: a `feat`/`fix` commit must add user-visible value of its own. A commit that doesn't (review fixups of a not-yet-released feature, second-layer wiring of a feature another commit already announced) is **amended into its feature commit** while still on the branch, or **typed `refactor`/`chore`/`test`/`docs`** so it never reaches the changelog. Two near-identical `feat`/`fix` titles in one release are the tell — reword or merge before release.
- **Style**: React functional components, Rust traits for repositories.
- **Lints**: Oxlint & Biome (FE), Clippy (BE), `scripts/arch-check.py` (both). All must pass.

## 🖼 Frontend Visual Proof

Full rules: `docs/frontend-visual-proof.md`

Any `.tsx`, `.css`, or visual asset change **must** include a committed screenshot in `screenshots/` before merging.

One-time setup: `npx playwright install chromium`

Run `/visual-proof` after any frontend change — generates previews for all component states in light + dark mode, captures with Playwright, and stages screenshots. `/design-proposal NNN` uses the same pipeline to render the **proposed** state into `screenshots/design/` before the work starts.

> **Modals**: render the panel directly without `ModalContainer` in `src/__preview__/main.tsx` — copy the `FormModal` chrome (rounded-[28px], `bg-m3-surface-container-lowest/85 backdrop-blur-[12px] shadow-elevation-4`, header / scrollable content / footer) and skip `ModalContainer`'s 50% scrim. The scrim is a generic shell concern with no real content behind it in a standalone preview, so it would render near-black and misrepresent the modal in dark mode.

> **No visual change**: write `No visual impact — internal refactor / Rust-only change.` at the top of the PR/commit, then screenshot a screen that _consumes_ the modified code as non-regression proof.

## ⚠️ Critical Patterns

### Tauri Service Layer - Gateway Pattern

All Tauri invocations in gateways MUST match `bindings.ts` signatures EXACTLY:

- ✅ `commands.addAsset(name, assetClass, categoryId, currency, riskLevel, reference)` - positional parameters
- ❌ `commands.addAsset({ name, assetClass, categoryId, currency, riskLevel, reference })` - object wrap (WRONG)
- **Rule**: Match parameter COUNT, ORDER, and NAMES from bindings.ts
- When binding has 5 params: call with 5 args in correct order, never wrapped

### Event bus — one notice per action

The bus is a `tokio::sync::watch` channel: two back-to-back `send()`s deliver only the second to every subscriber. An action publishes exactly one event; a removal publishes the event of the kind it removed, never a generic one followed by a specific one.

### Domain Entities - Factory & Aggregate-Root Methods

Domain objects expose two distinct families of methods. NEVER construct them via direct
struct literals outside these conventions.

**Factories** — produce a fresh aggregate. Static, do not take `self`:

- `new()` — generates a new ID + validates input
- `with_id()` — uses a caller-supplied ID + validates input (services / use cases / api)
- `from_storage()` (or `restore()`) — reconstructs from the database, no validation
  (already validated at write time)

**Mutating aggregate-root methods** — apply a state-dependent change to a loaded
aggregate. Instance methods, take `self` (or `&mut self`):

- `update_from(self, …fields) -> Result<Self, DomainError>` — applies an edit; enforces
  state invariants then validates input; returns the updated aggregate to persist
- `archive(self) / unarchive(self) -> Result<Self, DomainError>` — flips the archive flag;
  enforces invariants
- `ensure_<predicate>(&self) -> Result<(), DomainError>` — fail-fast guard used when the
  rejection must precede an action that doesn't construct a new aggregate (e.g. delete)

Rules for this family:

- Use **domain/business vocabulary** (per `docs/backend-rules.md` B11) — name the
  business action (`archive`, `cancel`), not the mechanism (`set_archived(true)`)
- Return typed **domain errors** directly (per `docs/ddd-reference.md` § Errors) — never
  `anyhow`
- All **state-dependent rejections** (`Archived`, `CashAssetNotEditable`,
  `SystemReadonly`, `SystemProtected`, etc.) MUST live here — not in the service
- The repository ONLY uses factories, never direct struct literals

---

## 📋 When the human asks for a plan

In a chat conversation (a feature being designed, a batch being scoped), a plan lists exact file paths and the functions/components to create or modify, separates layers (backend / frontend / E2E / docs), calls out any gold-conformance work with its LOC estimate, and includes the tests that will prove each clause. Once the human says go, the plan is the authority for the whole batch — no per-step confirmation.
