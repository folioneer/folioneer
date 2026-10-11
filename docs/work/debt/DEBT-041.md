# 2026-09-27 — DEBT-041 — Clickable table rows have no interactive element of their own

- Found by: the main agent (DEBT-039 review — nine E2E steps click `td:first-child` to open a row)
- Where: `src/features/currency/currency_rates_view/CurrencyRatesView.tsx` (`pair-row-*`), the account rows of `src/features/accounts/`, and every row whose `<tr>` carries `onClick`; the E2E specs clicking `… td:first-child`
- Severity: 🔵
- Observation: The rows are opened by an `onClick` on the `<tr>`, made focusable with `tabIndex` and a key handler. WebDriver cannot click a `<tr>` (its centre hit-tests to a cell), so specs click the first cell and rely on the event bubbling — a click on the wrong cell, or a cell that stops propagation, breaks them. Assistive technology meets a row announced as a row, not as a control that opens something.
- User value: Screen-reader and keyboard users meet a real link or button to open each row; the E2E suite clicks that control directly.
- Done when: each clickable row holds one link or button (with an `id` and an accessible name) that opens it, the row stays clickable for the mouse, and no E2E spec clicks `td:first-child`.
