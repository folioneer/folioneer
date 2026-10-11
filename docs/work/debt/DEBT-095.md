# 2026-10-10 — DEBT-095 — WebdriverIO 10 is available; the E2E tools are on 9

- Found by: the dependency audit before the 0.7.0 release
- Where: `package.json` — `webdriverio`, `@wdio/cli`, `@wdio/globals`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter` (9.31 installed, 10.0.2 published on 2026-10-08, three days after 10.0.0); `wdio.conf.ts`; `e2e/`
- Severity: 🟡
- Observation: the advisories left on the test tools (DEBT-092) sit on packages this chain brings, and `npm audit` clears them only with 10. The E2E suite does not run on this computer (`docs/lessons.md` L-011): CI is the only proof of the upgrade, about a quarter of an hour per attempt. 10 asks for Node 22.19 or later; the workflows ask for Node 22 without a minor, so the version they resolve is to check
- User value: None directly — the tool that drives the application in the E2E suite carries no known advisory.
- Done when: the six packages are at 10; the E2E suite is green in CI with no test skipped for the upgrade; `npm audit` no longer reports the advisories DEBT-092 names on this chain.
