---
name: dep-audit
description: Audit npm and Cargo dependencies for outdated versions and CVEs, with web-verified version data. Run at a release's hand-over, on the final `main`, or periodically during development.
tools: Bash, Read, WebSearch
---

# Skill — `dep-audit`

Outdated versions and known CVEs in `package.json` and `src-tauri/Cargo.toml`. A version is
never called latest or outdated from model knowledge, which may be months behind: each one
reported is checked with WebSearch and cited with its source URL. The skill never upgrades.

## 1. Read and run

The declared packages and ranges of both files, then each command on its own — a non-zero
exit is information (a missing tool, or findings), never a stop:

```bash
rustc --version
```

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

`just licence-check` is the project's own gate: a shipped dependency whose licence is outside
`licence-allowlist.json`. Then re-read `docs/external-dependencies.md` — the terms of the
services the application calls and the reference data copied into the source, which no tool
checks: report an item whose terms are unverified, or whose conditions the application no
longer meets.

A tool with no output and a non-zero exit is not installed: report it
(`⚠️ {tool} not installed — install with: cargo install {tool}`), never skip it silently.

## 2. Verify on the web

Every package flagged, and always: Rust stable (`rust stable release latest
site:blog.rust-lang.org`), Tauri (`tauri-apps tauri latest release github`), npm
(`{package} npm latest version`, npmjs.com), crates (`{crate} crates.io latest version`),
CVEs (`{package} security advisory CVE`, osv.dev).

## 3. Classify

- 🔴 **CVE** — `npm audit` severity `moderate`, `high` or `critical`; any `cargo audit` advisory.
- 🟡 **Major drift** — the latest major above the current one (v5 in use, v6 out).
- 🟡 **Misplaced** — a build-time package (bundler, linter, type checker, test runner, types)
  in `dependencies`, a runtime one (imported in `src/`) in `devDependencies`, a test-only
  crate in `[dependencies]`, two packages in one role. Reported even when current:
  placement changes the production bundle.
- 🔵 **Minor drift** — the latest minor above the current one, same major.

What a 🔴 CVE becomes is `docs/workflow.md` § 3's call. The report gives its evidence:
whether the application reaches the vulnerable code, read in the code (the advisory's
function is called, or never).

## 4. Report

```
## Dependency Audit — {date +%Y-%m-%d}

### Tool availability
⚠️ cargo-audit not installed — install with: cargo install cargo-audit
(omit when every tool is there)

### npm (`package.json`)
🔴 CVE: {package} {version} — {severity} — {advisory ID} — {description}
     Fixed in: {fixed-version} (source: {url}) — reached by the application: yes / no
🟡 Major drift: {package} {current} → {latest} (source: {url})
🟡 Misplaced: {package} in {wrong-section} — should be in {correct-section}
🔵 Minor drift: {package} {current} → {latest} (source: {url})

### Cargo (`src-tauri/Cargo.toml`)
🔴 CVE: {crate} {version} — {advisory} — {description} — reached by the application: yes / no
🟡 Major drift: {crate} {current} → {latest} (source: {url})
🔵 Minor drift: {crate} {current} → {latest} (source: {url})

### Rust toolchain
Installed: {rustc --version}
Stable:    {version from the web — source URL}
Status: ✅ up to date / 🟡 behind (raise `channel` in `rust-toolchain.toml`, in a pull request of its own)

### Summary
🔴 {N} CVE(s), {n} reached by the application
🟡 {N} major drift(s) / misplaced dep(s)
🔵 {N} minor drift(s)
Licences: ✅ / ❌ {packages} · External terms: ✅ / ⚠️ {items}
```

All clean: `✅ All dependencies up to date and no known CVEs.`
