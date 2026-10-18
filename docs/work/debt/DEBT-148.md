# 2026-10-18 — DEBT-148 — This batch's rules carry clauses no test pins

- Found by: `spec-checker` at the hand-over of the 2026-10-18 batch, on main at 7938e09
- Where: `agent_connection/end_to_end_tests.rs`, `use_cases/account_details/`, `use_cases/account_performance/`, `context/asset/`, `use_cases/shared/price_movement.rs`, `PriceMovementDialog.test.tsx`
- Severity: 🔵
- Size: M — tests only.
- Observation: AGT-040: through the tools, an outdated count above 0, `price_outdated` with `as_of`, `total_capital_brought_in`. AGT-048: a recording tool given an exponent-form figure (`2e-6` read, `2e-7` refused), `1e30` as a JSON number. ACD-055: a flow dated on the window start. ACD-057: a holding sold out before the window start and bought again. ACD-058: an opening balance dated after `as_of`. PRF-083: the period-over-period figure of a position bought inside the period. MKT-206: a read with a date gives `false`/`0`. MKT-155: archive keeps the typed-by-hand flag. MKT-156: the `AssetUpdated` event, an unknown asset, idempotency. PMV-011: one report only. PMV-028: colouring of the percentage cells.
- User value: Nothing visible — the figures and refusals shipped this batch stay as they are.
- Done when: a test per clause above, named with its rule.
