# 2026-10-10 — DEBT-093 — TypeScript 7 is available; the frontend is checked with 6

- Found by: the dependency audit before the 0.7.0 release
- Where: `package.json` — `typescript` (6.0.3 installed, 7.0.2 published on 2026-07-08); `tsconfig.json`, `tsconfig.node.json`; the `build` script (`tsc && vite build`)
- Severity: 🔵
- Observation: `tsc` is the type check of `src/` in the gate and the first step of the build. The E2E specs are not in its scope. The tools that read TypeScript beside it (Vite's React plug-in, Vitest, oxlint, Biome, the generated `src/bindings.ts`) are each to check against 7 before the move
- User value: None directly — the type check that guards the frontend runs on the current compiler.
- Done when: `typescript` is at 7; `just check --frontend` and the build pass with no rule of `tsconfig.json` loosened and no error silenced; the generated bindings compile unchanged.
