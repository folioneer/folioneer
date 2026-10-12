# 2026-10-12 — DEBT-123 — The journal is filtered by dates and by who recorded outside the account context

- Found by: `reviewer-arch` on TODO-074's pull request (2026-10-12), read again in the code by the main agent
- Where: `list_named` in `src-tauri/src/use_cases/holding_transaction/named_listing.rs`; `JournalFilter` in `src-tauri/src/context/account/domain/journal.rs`
- Severity: 🔵
- Size: M — one pull request: the account's journal filter, the bindings and the window's call of it.
- Observation: the account journal filters by asset, by transaction type and by total in the account context (`JournalFilter`, TXL-060); an agent's listing adds a range of dates and who recorded, applied in the use case after the journal is built. Two places decide which rows of a journal are kept, and the window's journal cannot ask for a range of dates. `JournalFilter` is a type of the window's bindings: moving the two filters there changes what the window sends
- User value: None directly; the window's journal could then be filtered by dates.
- Done when: a range of dates and who recorded are filters of the account journal itself, the agent's listing passes them, and one test holds each through the journal.
