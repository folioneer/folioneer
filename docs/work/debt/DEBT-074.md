# 2026-10-04 — DEBT-074 — A back-dated sale passes its draft check and is refused on save

- Found by: reviewer-backend on DEBT-070
- Where: `validate_draft` in `src-tauri/src/use_cases/holding_transaction/orchestrator.rs` (the oversell check of a new sale)
- Severity: 🔵
- Observation: the draft check compares the quantity sold with what is held today. A sale dated before later purchases can sell more than was held on its date: the draft is clean, and recording refuses it (`CascadingOversell`). The draft now returns no potential gain in that case, but reports no problem.
- User value: The sell dialog says before saving that the quantity was not held at that date.
- Done when: the draft check of a new sale refuses a quantity above the position as of the sale's date, with the error recording would give, shown on the quantity field.
