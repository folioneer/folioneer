# 2026-10-10 — DEBT-094 — Vitest 5 is available; the tests run on 4

- Found by: the dependency audit before the 0.7.0 release
- Where: `package.json` — `vitest`, `@vitest/coverage-v8` (4.1.11 installed, 5.0.3 published on 2026-09-30); `vitest.config.ts`
- Severity: 🔵
- Observation: the two packages move together. The coverage figure the gate reads (`coverage-gates.json`, frontend) comes from `@vitest/coverage-v8`: a major can count lines differently, so the figure is to compare before and after on the same commit. Vitest 5 accepts the Vite in use (8). Vite itself has no major waiting (8.0.16 → 8.3.4, a minor)
- User value: None directly — the frontend tests and their coverage figure run on a supported version.
- Done when: both packages are at 5; every frontend test passes; the frontend coverage figure before and after is written in the pull request, and a move of the figure is explained, the floor never lowered.
