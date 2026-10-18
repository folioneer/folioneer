# 2026-10-18 — DEBT-149 — Code still says "lock" for a price typed by hand

- Found by: `spec-checker` at the hand-over of the 2026-10-18 batch, on main at 7938e09
- Where: `use_cases/shared/price_movement.rs:98, 551-585`, `asset_price_fetch/movement_capture.rs:225`, `use_cases/shared/scope.rs:2, 45, 145, 176`, `src/lib/store.ts:84`
- Severity: 🔵
- Size: S — words only.
- Observation: The vocabulary forbids "lock" (`docs/ubiquitous-language.md`, TODO-109); comments, a test name and the `asset-locked` fixtures still use it.
- User value: Nothing visible.
- Done when: `grep -rni lock` on those files finds only unrelated uses; the tests pass unchanged.
