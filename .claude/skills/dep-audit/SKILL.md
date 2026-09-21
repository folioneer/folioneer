---
name: dep-audit
description: Audit npm and Cargo dependencies for outdated versions and CVEs, with web-verified version data. Run before every release or periodically during development.
tools: Bash, Read, WebSearch
---

# Skill — `dep-audit`

Audit npm and Cargo dependencies for outdated versions and known CVEs.
Uses **live web search** to verify real-world versions — never relies on model knowledge alone.

---

## Required tools

This skill requires: `Bash`, `Read`, `WebSearch`.

**Before starting**: verify that `WebSearch` is available. If it is not, stop and inform the user — without web search, version data would come from stale model knowledge and cannot be trusted.

---

## When to use

- **Before every release** — CVEs are a release blocker
- **Start of a work session** (optional) — surface version drift early
- **After a batch of dependency changes** — confirm no new vulnerabilities introduced

---

## Placement rules (applies always, no tools needed)

| Finding                              | Rule                                                                                            |
| ------------------------------------ | ----------------------------------------------------------------------------------------------- |
| Build-time package in `dependencies` | 🟡 Move to `devDependencies` (bundlers, linters, type checkers, test runners, type definitions) |
| Runtime package in `devDependencies` | 🟡 Move to `dependencies` (UI libs, state managers, utilities imported in `src/`)               |
| Test-only crate in `[dependencies]`  | 🟡 Move to `[dev-dependencies]`                                                                 |
| Two packages serving the same role   | 🟡 Keep one, remove the other                                                                   |

---

## Execution Steps

### 1. Read declared dependencies

Read `package.json`. Locate `Cargo.toml` by reading `ARCHITECTURE.md` or by searching for `Cargo.toml` in the project root and one level of subdirectories; skip Cargo checks if not found. Extract all declared packages and their version ranges.

### 2. Get current toolchain version

```bash
rustc --version
```

### 3. Run local audit tools

Run the following — failures are expected if tools are not installed and must appear in the report under "Tool availability":

```bash
npm outdated 2>/dev/null
```

```bash
npm audit 2>/dev/null
```

```bash
cargo outdated 2>/dev/null
```

```bash
cargo audit 2>/dev/null
```

```bash
just licence-check
```

`just licence-check` is the project's own gate, not an optional tool: it lists every shipped dependency whose licence is outside `licence-allowlist.json`. CI runs it on every pull request, so a failure here means `main` moved since, or the lockfiles changed locally.

Then re-read [`docs/external-dependencies.md`](../../../docs/external-dependencies.md). `licence-check` reads the licence of what the application **ships**; that document records the terms of the services it **calls** and the reference data copied into the source, which no tool can check. Report any item whose terms are marked unverified, and any whose conditions the application has stopped meeting (the ECB asks to be cited as the source, and the rate rows do it).

Non-zero exit is expected when tools are missing or issues are found — treat the output as informational and continue. If a command produces no output and exits non-zero, note the tool as unavailable in the report.

If `cargo-outdated` or `cargo-audit` are missing, flag them in the report: `⚠️ {tool} not installed — install with: cargo install {tool}`

### 4. Web-verify all reported issues

**CRITICAL**: Never report a version as "latest" or "outdated" based solely on model knowledge — it may be months out of date. For every package flagged as outdated, use WebSearch to confirm the actual current version and cite the source URL.

Always verify:

- **Rust stable**: search `rust stable release latest site:blog.rust-lang.org`
- **Tauri**: search `tauri-apps tauri latest release github`
- **npm packages**: search `{package} npm latest version` or check npmjs.com
- **Cargo crates**: search `{crate} crates.io latest version`
- **CVEs**: search `{package} security advisory CVE 2025` or check osv.dev

### 5. Classify findings

| Severity       | Criteria                                                                                              |
| -------------- | ----------------------------------------------------------------------------------------------------- |
| 🔴 CVE         | `npm audit`: severity `moderate`/`high`/`critical`. `cargo audit`: any advisory. **Release blocker.** |
| 🔴 Licence     | `just licence-check` fails: a shipped dependency outside the allow-list. **Release blocker.**         |
| 🟡 Major drift | Latest major > current major (e.g. v5 in use, v6 available)                                           |
| 🟡 Misplaced   | See placement rules above                                                                             |
| 🔵 Minor drift | Latest minor > current minor within same major                                                        |

**Release vs. non-release context:**

- In **release context**: 🔴 CVEs are hard blockers — do not proceed to tag/publish without fixing.
- In **non-release context**: 🔴 CVEs are flagged urgently but do not block the current session — inform the user and recommend scheduling a fix.

### 6. Report

Get today's date (`date +%Y-%m-%d`) for the report heading.

```
## Dependency Audit — {date}

### Tool availability
⚠️ cargo-outdated not installed — install with: cargo install cargo-outdated
⚠️ cargo-audit not installed — install with: cargo install cargo-audit
(omit this section if all tools are available)

### npm (`package.json`)
🔴 CVE: {package} {version} — {severity} — {advisory ID} — {description}
     Fix: npm audit fix (or manual bump to {fixed-version}, source: {url})
🟡 Major drift: {package} {current} → {latest} (source: {url})
🟡 Misplaced: {package} in {wrong-section} — should be in {correct-section}
🔵 Minor drift: {package} {current} → {latest} (source: {url})

### Cargo (`Cargo.toml`)
🔴 CVE: {crate} {version} — {advisory} — {description}
🟡 Major drift: {crate} {current} → {latest} (source: {url})
🔵 Minor drift: {crate} {current} → {latest} (source: {url})

### Licences (`licence-allowlist.json`)
✅ {N} crates and {N} npm packages ship, all under allowed licences ({N} by name)
🔴 Licence: {package} — {licence} (replace it, or record a deliberate decision in licence-allowlist.json)

### Services and embedded data (`docs/external-dependencies.md`)
✅ {N} services and {N} embedded data sets recorded; terms last read {date}
🟡 {service} — terms unverified since {date}; read them before anything is sold
🔴 {service} — a condition the application no longer meets: {condition}

### Rust toolchain
Installed: {output of rustc --version}
Stable:    {version from web — cite source URL}
Status: ✅ up to date / 🟡 behind (update with: rustup update stable)

### Summary
🔴 {N} CVE(s) — must fix before release
🟡 {N} major drift(s) / misplaced dep(s)
🔵 {N} minor drift(s)
```

If all clean: `✅ All dependencies up to date and no known CVEs.`

---

## Critical Rules

1. **Always use WebSearch to verify versions** — never report stale model knowledge as fact
2. **Always cite the source URL** next to any version number you report
3. 🔴 CVEs are a **release blocker** — do not proceed to tag/publish without fixing
4. Missing tools (`cargo-audit`, `cargo-outdated`) must be flagged in the report, not silently skipped
5. Report `misplaced` deps even if versions are current — placement affects production bundle size

---

## Notes

Web search is mandatory because model training data can be months behind the actual package ecosystem. A missed CVE or a falsely "current" version is worse than not running the audit at all. The skill is designed to be trustworthy, not fast.
