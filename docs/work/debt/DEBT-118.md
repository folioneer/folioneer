# 2026-10-11 — DEBT-118 — The vocabulary's table of transaction types has no row for a split

- Found by: `spec-reviewer` on `docs/spec/agent-connection.md` during TODO-071 (2026-10-11), read again in `docs/ubiquitous-language.md` by the main agent
- Where: `docs/ubiquitous-language.md` § Transaction Types
- Severity: 🔵
- Size: S — one pull request: one row of the vocabulary, validated by the owner.
- Observation: the table lists `Purchase`, `Sell`, `OpeningBalance`, `Deposit`, `Withdrawal`, `Dividend`, `FreeShares`, `ManagementFee` and `Interest`; `TransactionType::Split` exists in the code and "Stock Split" is a confirmed term of the same document, with no row in the table
- User value: None directly.
- Done when: the table has a row for `Split`, in words the owner validated.
