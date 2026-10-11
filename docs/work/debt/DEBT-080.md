# 2026-10-04 — DEBT-080 — The E2E tooling carries 22 known advisories

- Found by: `/dep-audit` before the 0.6.0 release
- Where: `package.json` — the WebdriverIO packages (`@wdio/*`, `webdriverio`, 9.31.7) and what they bring: `undici` (< 7.29.1), `basic-ftp`, `extract-zip`, `braces`
- Severity: 🔵
- Observation: `npm audit` lists 22 advisories (18 high), all in development dependencies; `npm audit --omit=dev` finds none, so nothing of it ships. `npm audit fix` moves WebdriverIO to 9.32.0 and leaves 20: the chain pins the affected versions, and the only complete fix offered is a downgrade to WebdriverIO 8. The tooling runs on CI and on this machine against the application under test, never against untrusted input.
- User value: None directly — the audit before a release reads clean again.
- Done when: `npm audit` reports no advisory, by a WebdriverIO release that drops the affected versions or by `overrides` proven on a green E2E run; or each one left is recorded with why it cannot be reached.
