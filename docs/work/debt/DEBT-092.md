# 2026-10-10 — DEBT-092 — The test tools carry about twenty high advisories; six majors are available

- Found by: the dependency audit before the 0.7.0 release
- Where: `package-lock.json` — packages reached only through `devDependencies` (the WebdriverIO chain: `basic-ftp`, `extract-zip`, `braces`, `brace-expansion`, `ip-address`, `undici`, `serialize-javascript`, `diff`); `src-tauri/Cargo.toml`
- Severity: 🟡
- Observation: `npm audit --omit=dev` reports nothing: none of these packages ships. `npm audit` reports about twenty high advisories on the tools that run the E2E suite and the tests, on the developer's computer and on the CI runner. `npm audit fix` rewrites about 1,200 lines of the lock file and leaves twenty; the rest asks for WebdriverIO 10. Majors available, read from the registries on 2026-10-10: WebdriverIO 10 (DEBT-095), Vitest 5 (DEBT-094), TypeScript 7 (DEBT-093), each with its own entry; here, `@testing-library/jest-dom` 6 → 7; `sqlx` 0.8 → 0.9, `argon2` 0.5 → 0.6 with `password-hash` 0.6, `sha2` 0.10 → 0.11, `dirs` 6 → 7, `iso_currency` 0.5 → 0.7, `strum` 0.27 → 0.28. `cargo audit` reports no vulnerability; nine warnings on crates brought by others (unmaintained `paste`, `proc-macro-error`, `unic-*`; unsound `glib` 0.18; yanked `spin` 0.9.8)
- User value: None directly — the tools that prove the application carry no known advisory.
- Done when: `npm audit` reports no high or critical advisory, or each one left is named with the reason it cannot be reached; each major named here is taken or has a line saying what holds it.
