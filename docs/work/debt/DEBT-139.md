# 2026-10-18 — DEBT-139 — `codecov.yml` sits at the root where nothing requires it

- Found by: the owner's question on the files at the root (2026-10-18), read against what each tool requires
- Where: `codecov.yml`
- Severity: 🔵
- Size: S — one pull request: a rename.
- Observation: of the 26 files Git tracks at the root, `codecov.yml` is the one a move costs nothing: no script, workflow or doc names it (`git grep codecov.yml` finds none), and Codecov reads its configuration from `.github/codecov.yml` as from the root. The 14 the tools require there (npm, TypeScript, Vite, Vitest, WebdriverIO, Biome, oxlint, rustup, `just`, Git) and the conventional documents (README, LICENSE, CHANGELOG, CONTRIBUTING, CLA, CLAUDE.md, ARCHITECTURE.md) stay; the five settings of the project's own checks are DEBT-140.
- User value: None directly.
- Done when: `codecov.yml` is `.github/codecov.yml`, and the next pull request's Codecov report still applies it (the `codecov/patch` check reports, with the same targets).
