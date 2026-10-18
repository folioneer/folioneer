# 2026-09-14 — DEBT-025 — "Reference currency" names two different things in the vocabulary

- Found by: spec-reviewer (ACC-027–033 review on TODO-007)
- Where: `docs/ubiquitous-language.md` (Cash Holding, Dividends received, Management fees — "the account's reference currency"), `docs/spec/global-performance.md` GPF-011 and the ACC / PMV totals ("the reference currency", EUR); the same table is also "Accounts table" (ACC-021/023/026), "accounts list" (PMV-013/016, SYN-040) and "account table" (ACC-008/030–033)
- Severity: 🔵
- Narrowed (2026-10-18, TODO-032): "account currency" and "reference currency" are now distinct confirmed entries and the specs use them; "Portfolio total" was confirmed the same day; what remains is one name for the accounts table, and the price-movement spec's "Portfolio total" section (PMV-040–045), which names the movement report's total, not the accounts list's.
- Observation: the vocabulary uses "the account's reference currency" for an account's own currency, while GPF-011, the price movement total and the accounts-list portfolio total use "the reference currency" for the fixed EUR every cross-account figure is reported in; the two meanings share one phrase, and neither "cross-account reference currency" nor "portfolio total" has an entry of its own.
- User value: None — one word per concept in the specs and the code.
- Done when: the accounts table has one name across the specs ("account table", "accounts table", "accounts list"), validated by the human.
