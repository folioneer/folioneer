# Commit Policy & Versioning

This document outlines the commit standards, workflow, and versioning strategy for the Midwife Patient Management System project.

## Overview

We follow a **conventional commits** approach with automated quality checks to maintain code consistency and enable semantic versioning for automated releases. Each commit type directly maps to semantic version bumps.

## Commit Format

All commits must follow this format:

```
type: description
```

### Type

Must be one of the following:

| Type       | Purpose                                 | Example                                       |
| ---------- | --------------------------------------- | --------------------------------------------- |
| `feat`     | A new feature                           | `feat: add patient search functionality`      |
| `fix`      | A bug fix                               | `fix: correct date validation in forms`       |
| `docs`     | Documentation changes                   | `docs: update README with setup instructions` |
| `test`     | Adding or updating tests                | `test: add unit tests for patient service`    |
| `chore`    | Build, dependencies, or tooling         | `chore: upgrade React to 18.3`                |
| `refactor` | Code restructuring (no behavior change) | `refactor: extract validation logic`          |

### Description

- Use **imperative mood** ("add" not "added" or "adds")
- Keep it **concise** (under 72 characters recommended for title)
- Use **lowercase**
- Do **not** end with a period
- Be **specific** about what changed and why, not how
- **Title**: Max 72 characters
- **Optional body**: Max 5 lines for context (rationale, what changed, breaking changes)
  - Do NOT include test results (those go in PR descriptions only)
- **No co-authoring footers** (never include `Co-Authored-By:` lines)

### Commit Messages vs. PR Descriptions

**Commits** describe WHAT changed and WHY (max 5 lines, no test results):

```
feat: add patient tracking fields

Populate latestProcedureType and latestFund when creating procedures.
Improves UX by reducing required data entry.
```

**Pull Requests** provide context and VALIDATION (max 10 lines, include test results):

```
## Summary
Add latestProcedureType, latestFund, latestDate fields to Patient.
Automatically populate recent service type and fund when creating procedures.

## Tests
- All 39 tests passing
- No linting errors
```

## Semantic Versioning (SemVer)

Commit types automatically determine version bumps following SemVer:

**Version format:** `MAJOR.MINOR.PATCH` (e.g., `1.2.3`)

| Commit Type                         | Version Change           | Example                         |
| ----------------------------------- | ------------------------ | ------------------------------- |
| `feat`                              | MINOR bump               | `1.0.0` → `1.1.0` (new feature) |
| `fix`                               | PATCH bump               | `1.0.0` → `1.0.1` (bug fix)     |
| `docs`, `chore`, `test`, `refactor` | PATCH bump or no release | No direct version change        |

**SemVer Definitions:**

- **MINOR**: New features added (backwards compatible)
- **PATCH**: Bug fixes and patches (backwards compatible)

**Note:** MAJOR version bumps are handled manually when introducing incompatible API changes.

#### Good Examples

- `feat: add patient appointment scheduling`
- `fix: handle null values in reimbursement calculator`
- `docs: add database schema documentation`
- `test: add integration tests for Tauri bridge`
- `refactor: consolidate duplicate validation logic`

#### Bad Examples

- `Updated stuff` - Too vague
- `FEAT: Add patient search` - Wrong capitalization
- `fix: Fixed the bug.` - Unnecessary period, passive voice
- `chore: upgrade deps and fix linter and update readme` - Too many things
- `feat: add patient search\n\nCo-Authored-By: John Doe <john@example.com>` - Never use co-author footers

## Pre-Commit Checklist

Before committing, ensure:

### 1. Tests Pass ✅

```bash
npm run test
```

All tests must pass. If a test fails, fix the issue before committing.

### 2. Code Quality ✅

Run the linters appropriate to your changes:

**Frontend (JavaScript/TypeScript):**

```bash
npm run lint
```

Auto-fix issues when possible:

```bash
npm run lint:fix
```

**Backend (Rust):**

```bash
cd src-tauri
cargo clippy -- -D warnings
```

Linter errors must be fixed before committing. Minor warnings can be acknowledged if documented.

### 3. No Sensitive Data ✅

Never commit files containing:

- `.env` files with credentials
- Private keys (`.key`, `.pem`)
- API tokens or secrets
- Database passwords

### 4. Logical Changes ✅

Each commit should represent a single logical change. If you're tempted to say "and" in your commit message, you might need multiple commits.

## Approval Requirement ✅

**Always ask for approval before committing.** Do not commit changes without explicit user consent.

## Commit Workflow

### Using the Smart Commit Tool

The easiest way to create a compliant commit:

```bash
/commit
```

This tool will:

1. Show you what files have changed
2. Check for sensitive files
3. Run tests and linters
4. Suggest an appropriate commit type
5. Guide you through writing the message
6. Create the commit with proper formatting

### Manual Commit

If you prefer to commit manually:

```bash
# 1. Check your changes
git status

# 2. Run quality checks
npm run test
npm run lint

# 3. Stage your changes
git add <files>

# 4. Commit with proper format
git commit -m "type: description"
```

## Examples

### Adding a Feature

```bash
# 1. Implement feature and tests
# 2. Run quality checks
npm run test
npm run lint

# 3. Commit
/commit
# Select: feat
# Message: "add patient appointment scheduling"
```

### Fixing a Bug

```bash
# 1. Fix the issue and add test for regression
# 2. Run quality checks
npm run test
npm run lint

# 3. Commit
/commit
# Select: fix
# Message: "handle null values in reimbursement calculator"
```

### Updating Documentation

```bash
# 1. Update README or docs
# 2. Commit
/commit
# Select: docs
# Message: "update database schema documentation"
```

## Breaking Changes

Breaking changes that require a MAJOR version bump should be handled through a manual release process. Document incompatible API changes in the commit body for reference:

```
feat: redesign patient data model

This change modifies the patient API response format.
Requires manual version bump to next major version.
```

## Release Process

### Automated Releases

Releases are automated using the release script that analyzes commits since the last tag:

```bash
just release
```

Or dry-run mode to preview changes:

```bash
just release-dry
```

The script will:

1. Run all tests (React + Rust)
2. Analyze commit history since last tag
3. Calculate version bump using SemVer rules
4. Update version files (`package.json`, `Cargo.toml`, `tauri.conf.json`)
5. Generate CHANGELOG entry
6. Create commit and git tag
   - **Note:** If git hooks are configured (`git config core.hooksPath .githooks`), the pre-commit hook will run all quality checks (tests, linting, formatting) before allowing the release commit. This ensures releases only happen from clean code.

See [Contributing Guidelines](./CONTRIBUTING.md) for git hook setup.

**Example flow:**

- 2 `feat` commits, 3 `fix` commits since v1.0.0 → releases v1.1.0
- Only `docs`, `chore` commits → no automatic release
- Breaking changes → manual MAJOR version bump

### Manual Release (if needed)

```bash
# 1. Update version in files following SemVer
# 2. Create commit
git commit -m "chore: release v1.1.0"

# 3. Tag the commit
git tag -a v1.1.0 -m "Version 1.1.0"

# 4. Push
git push && git push --tags
```

## Related Documentation

- [Contributing Guidelines](./CONTRIBUTING.md) - General contribution workflow
- [Testing Strategy](./TESTING.md) - Testing requirements
- [Architecture](./ARCHITECTURE.md) - System design and structure

## Quality Assurance

All commits go through automated checks:

- ✅ Tests must pass
- ✅ Linters must report no critical errors
- ✅ Commit format must be valid

These checks are enforced by the `commit-msg` hook and again by the `pr-checks` job in CI.

## Troubleshooting

### "Linter reported errors"

Fix the linting issues:

```bash
npm run lint:fix      # Auto-fix fixable issues
cargo clippy -- -D warnings  # Check Rust code
```

Then commit again.

### "Tests failed"

See [Testing Strategy](./TESTING.md) for debugging help.

### "Commit message too long"

The recommended limit is 72 characters. Try to be more concise:

- ❌ `feat: add ability for users to search and filter patients by various criteria including name date and status`
- ✅ `feat: add patient search and filtering`

### "Can't remember commit types"

The `/commit` smart tool will suggest the right type based on your changes. Just run:

```bash
/commit
```
