---
name: prune
description: Audit the project for structural simplification — fewer lines, methods, and unnecessary code — without touching architecture or DDD structure. Coverage report is mandatory; refuses to proceed without one. Read-only; produces a prioritized report for human review.
tools: Bash, Read, Grep, Glob, Write
---

# Skill — `prune`

Audit the project for code that can be removed or collapsed without changing architecture, business logic, or DDD structure. Targets three categories only: dead code, pass-through methods, and verbose patterns.

**This skill is read-only.** It produces a report for human review and never edits files.

**Coverage is a hard gate.** No coverage report = full stop. Suggesting changes to untested code is not an acceptable risk.

> **Model recommendation:** Run this skill on Opus. The confidence classifications require judgment that smaller models handle poorly.

---

## Invocation

```
/prune [path]
```

`[path]` is optional. When provided, limits the scan to that directory (e.g. `/prune src/features/auth`). When omitted, scans all source files.

---

## When to use

- After a feature ships — to trim bloat that accumulated during development
- Before a refactor — to reduce surface area first
- As a periodic KISS health check

---

## Execution steps

### Step 0 — Coverage gate (hard stop)

Search for a coverage report with the `Glob` tool — one call per pattern,
ignoring any hit under `node_modules/` or `target/`:

- `**/lcov.info`
- `**/coverage-final.json`
- `**/cobertura.xml`
- `**/tarpaulin-report.html`

(A `Glob` call keeps discovery on the auto-allow path; a `find | grep | grep`
pipeline would prompt for permission on every invocation.)

**If no report is found → STOP immediately.** Output exactly:

```
❌ No coverage report found. /prune cannot safely suggest simplifications without one.

Generate a report first:
  Rust:    cargo install cargo-tarpaulin && SQLX_OFFLINE=true cargo tarpaulin --out Lcov Html --output-dir coverage/rust --lib --exclude-files "build.rs"
  Jest:    jest --coverage
  Vitest:  vitest run --coverage

Re-run /prune after generating a report.
```

Do not proceed. Do not produce partial findings.

If a report is found, note its path and format, then continue.

---

### Step 1 — Compute REPORT_PATH

Run `bash scripts/report-path.sh prune` and store the output as `REPORT_PATH`.

---

### Step 2 — Discover source files

If a `[path]` argument was given, scan only that directory. Otherwise, locate source directories from `ARCHITECTURE.md` or by searching for common roots (`src/`, `infra/`, `lib/`).

Discover candidates with the `Glob` tool (e.g. `{path}/**/*.ts`, `**/*.tsx`, `**/*.rs`), then drop, in context, any path that is a test or generated file or sits under `node_modules/`, `target/`, or `.git/` — `__tests__/`, `.test.`, `.spec.`. `Glob` already returns sorted results, so no `find | grep | sort` pipeline (which would prompt on every run) is needed.

---

### Step 3 — Scan each file for simplification targets

Read each source file fully before scanning. Check all four categories.

#### Category A — Dead code

Private/internal symbols (functions, methods, constants, types) with zero references anywhere in the project.

Detection:

1. Identify private/internal symbols in the file (not exported, not `pub` in Rust).
2. `Grep` for the symbol name across all source files.
3. If references exist only in the declaration file itself → candidate.

Confidence rules:

- `certain` — private/internal symbol, zero external references
- `possible` — exported/public symbol with zero internal references (may be consumed outside the repo)

Never mark an exported symbol as `certain` dead code.

#### Category B — Pass-through methods

A method whose entire body is a single delegation to one other method with no transformation, no guard, and no added domain meaning.

```typescript
// pass-through — adds nothing
getUser(id: string) { return this.userRepository.getUser(id); }
```

Confidence rules:

- `certain` — no interface or trait is being satisfied by this method
- `possible` — an interface or trait match is found (the delegation may be required structurally)

Verify interface/trait compliance before classifying. In Rust, check `impl Trait for`. In TypeScript, check `implements`.

#### Category C — Verbose patterns

Multi-line constructs that collapse to fewer lines through a mechanical rewrite with no semantic ambiguity:

- A 4+ line if/else that returns a boolean → single expression
- A manual loop that builds an array → `map`/`filter`/`reduce`
- A multi-step null-check chain → optional chaining (`?.`) or Rust `?` operator
- An immediately-invoked closure/lambda that just wraps a value with no logic

Only flag when the rewrite is unambiguous — if you have to think about whether semantics are preserved, skip it.

Confidence: always `certain` (if it requires judgment, it does not qualify).

#### Category D — Duplicate definitions

A function or constant defined under the same name in two or more non-test source files, where the implementations are substantially similar.

Detection:

1. Extract all top-level definition names from the discovered source files.
2. For each name, use the `Grep` tool to find its definition (not just usage) across the project. Scope by file type and read the matches in context — no shell `grep | grep -v` pipeline (it would prompt on every call):
   - **TypeScript/TSX** — pattern `^\s*(export )?(function|const|async function) {SYMBOL}`, `glob: "*.{ts,tsx}"`; ignore matches in `.test.` / `.spec.` / `__tests__` paths.
   - **Rust** — pattern `^\s*(pub )?(fn|const) {SYMBOL}`, `glob: "*.rs"`; ignore matches under `/tests/`.
3. If a name appears as a definition in 2+ files → read both implementations fully → compare bodies.
4. Only flag when implementations are **substantially similar** (same logic, possibly different variable names or minor adaptations). Skip if the bodies serve clearly different purposes despite sharing a name.

Confidence rules:

- `possible` — always. Intentional parallel implementations exist (e.g. same method name on two different domain services). Never upgrade to `certain`.

Exclude:

- Test files and test helpers
- Interface/trait implementations — same method name on different `impl` blocks is structural, not duplication
- Getters/setters (too short to judge similarity meaningfully)

---

### Step 4 — Coverage check per finding

For each candidate from Step 3, determine whether the relevant code is covered. Use the appropriate command for the detected report format.

#### lcov (`lcov.info`)

```bash
# Get hit/found line counts for a specific file
awk -v file="src/foo.ts" '
  /^SF:/ { in_file = ($0 ~ file) }
  in_file && /^LH:/ { lh = substr($0, 4) }
  in_file && /^LF:/ { lf = substr($0, 4) }
  in_file && /^end_of_record/ { print lh "/" lf; in_file = 0 }
' lcov.info
# Output: "hits/total" — if hits == total → ✅; if hits == 0 → ❌; otherwise → ❓
```

#### tarpaulin HTML (`tarpaulin-report.html`)

Human-readable report only — no machine-parseable coverage data. When this is the only Rust report present alongside `lcov.info`, use the lcov section above for per-file coverage. The HTML is for manual inspection.

#### Jest JSON (`coverage-final.json`)

```bash
jq --arg file "src/foo.ts" '
  to_entries[]
  | select(.key | endswith($file))
  | { hit: ([.value.s[] | select(. > 0)] | length), total: (.value.s | length) }
' coverage-final.json
# Output: hit/total — if hit == total → ✅; if hit == 0 → ❌; otherwise → ❓
```

Mark each finding:

- ✅ **Covered** — all lines hit → safe to recommend
- ❌ **Uncovered** — zero hits → report as warning only, do not recommend
- ❓ **Partial** — some hits, some not → report with caution note; do not recommend without explicit user acknowledgment
- **Unknown** — file absent from report → treat as ❌

---

### Step 5 — Output, save, confirm

Print findings to the conversation, save to `REPORT_PATH` with Write, then reply: `Report saved to {REPORT_PATH}.`

---

## Output format

```
## Prune Audit — {date}

Coverage report: {path}
Scope: {directory or "full project"}
Files scanned: {N} · Findings: {N} recommended / {N} skipped (uncovered/partial)

---

### A — Dead code ({N})

#### `{SymbolName}` — {file}:{line}
Confidence: certain
Coverage: ✅ covered (42/42 lines hit)
Suggestion: Remove — zero references outside this file.

---

### B — Pass-through methods ({N})

#### `{MethodName}` — {file}:{line}
Confidence: certain
Coverage: ✅ covered
Delegates to: `{target}`
Suggestion: Remove wrapper; call `{target}` directly at {N} call site(s).
Call sites: {file}:{line}, {file}:{line}

---

### C — Verbose patterns ({N})

#### {file}:{line}
Confidence: certain
Coverage: ✅ covered
Before ({N} lines):
  {code}
After (1 line):
  {collapsed}

---

### D — Duplicate definitions ({N})

#### `{SymbolName}` — defined in {N} files
Confidence: possible
Coverage: ✅ covered (all definitions)
Definitions:
  {file-a}:{line} — {body or first 3 lines}
  {file-b}:{line} — {body or first 3 lines}
Suggestion: Extract to a shared module; replace both with a single import.

---

### Skipped (uncovered or partial)

| File | Symbol / Line | Category | Coverage |
|------|---------------|----------|----------|
| src/foo.ts | `doThing` | A | ❌ uncovered — not safe to recommend |
| src/bar.ts | line 88 | C | ❓ partial — verify before acting |

---

### Summary

| Category | Certain | Possible | Skipped |
|----------|---------|----------|---------|
| A Dead code | N | N | N |
| B Pass-through | N | N | N |
| C Verbose | N | — | N |
| D Duplicates | — | N | N |

> All `certain` findings are mechanical — safe to apply without risk.
> `Possible` findings need human judgment before acting.
> To apply a change, reference the finding (e.g. "apply prune finding B-2") in a follow-up message.
```

If nothing found: `✅ No simplification opportunities found. Codebase looks lean.`

---

## Critical rules

1. **Coverage gate is non-negotiable.** No report → refuse entirely. Partial/uncovered findings are warnings, never recommendations.
2. **This skill never edits files.** Read-only. Always.
3. **DDD symbols are protected.** Any symbol whose name carries domain meaning (entity, value object, aggregate, domain event, domain service, repository interface) is out of scope regardless of size or reference count.
4. **`Possible` findings must show call sites.** Never flag a public symbol without showing where it's used — the human needs full context to judge.
5. **Category C requires zero ambiguity.** If the collapsed form could change behavior in any edge case, skip the finding entirely.
6. **Four categories only.** Do not expand scope into style, naming, architecture, or layer structure — that is not this skill's job.
7. **Category D is always `possible`.** Same-name definitions on different domain objects are often intentional. Never upgrade duplicate findings to `certain`.
