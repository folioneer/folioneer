# 2026-10-10 — DEBT-104 — Four specs still describe a "Record" dropdown the header no longer has

- Found by: `spec-reviewer` in CI on pull request 183 (DEBT-100), where FEE-010 was amended to the window
- Where: `docs/spec/cash-dividend.md` (DIV-010, DIV-012, its workflow, UX draft and user flow), `docs/spec/free-share-distribution.md` (FSD-010, its workflow, UX draft and user flow), `docs/spec/cash-tracking.md` (the note under CSH-019), `docs/spec/financial-asset-transaction.md` (the note "Superseded by DIV-012")
- Severity: 🔵
- Observation: these rules say the Account Details header groups its record actions under one "Record" dropdown menu. The header carries a row of direct buttons: INT-010 says so, FEE-010 now does, and `e2e/helpers/navigation.ts` (`clickHeaderAction`) notes that the dropdown was flattened. Not read: the header component itself, for which actions it carries today
- User value: None — the rules a screen is checked against say what the screen shows.
- Done when: DIV-012 says how the header's record actions are laid out and lists them as the header has them; the passages of the four specs that name the dropdown follow it.
