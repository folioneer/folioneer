# 2026-10-13 — DEBT-129 — The E2E suite runs on this machine again, and the rules still say it cannot

- Found by: the main agent, at the owner's question on 2026-10-12: `npm run test:e2e:ci` on `main` after the 0.10.0 release — 23 spec files of 23 passed, 51 tests, in about two minutes, exit 0 (`tmp/e2e-local-open-balance.log`), the specs that write through IPC among them (an opening balance, a purchase and a sale, cash, a split, enabling sync). One run, not repeated
- Where: `CLAUDE.md` § Commands ("the local run is broken on this machine"); `docs/lessons.md` L-011; `package.json` (`test:e2e`, `test:e2e:ci`); `justfile` (no recipe)
- Severity: 🟡
- Size: S — one pull request: a recipe, two rule lines, a line of a lesson.
- Observation: L-011 records, from 2026-08-23, that every E2E spec writing through IPC timed out on this machine while CI was green, and `CLAUDE.md` says the local run is broken. It passed on 2026-10-12 with WebKitGTK 2.52.6; what changed since August was not looked for, and one green run does not say it holds under load, which L-011 named. The suite has no recipe: it was started with the native command, and a `--spec` argument given after `npm run test:e2e:ci --` did not reach the runner — the whole suite ran. DEBT-095 (WebdriverIO 10) says the upgrade cannot be tried locally for this reason
- User value: None directly; an E2E failure is seen before a push and not one round later. The owner asked for it before the next batch runs (2026-10-12).
- Done when: the suite has run green here three times in a row, or what makes it fail is written; a recipe starts it, one spec or all, the argument reaching the runner; `CLAUDE.md` and L-011 say what is true of this machine today; whether the local harness runs it before a push is the owner's to say, and is said.
- **Open questions:**
  - [x] Does the local harness run the E2E suite before a push? — Never by default: a recipe runs it by hand, one spec or all, and CI stays the E2E gate (owner, 2026-10-17).
