# 2026-10-18 — DEBT-143 — The account's country and wrapper lack the tests that pin their edges

- Found by: `spec-checker` at the hand-over of the 2026-10-18 batch, on main at 7938e09
- Where: `src-tauri/src/context/account/` (service, domain), `agent_connection/end_to_end_tests.rs`, `src/features/accounts/edit_account_modal/`, `use_cases/account_creation/orchestrator.rs`
- Severity: 🔵
- Size: S — tests only.
- Observation: ACC-040–044 are implemented; untested: an edit leaving a value empty clears it; an edit moving the account to a country whose list lacks its wrapper is refused with nothing changed; the edit hook sends the stored values back (`useEditAccountModal.ts:70-72`); `list_accounts` with an account's own country and a wrapper (ACC-042); the `get_wrapper_lists` payload; account creation refusing a wrapper seeds no cash line. Older: no direct test for ACC-005/006 (the cascade), ACC-007 (sorted by name), ACC-015 (ManualMonth default), ACC-022 (AccountUpdated sent).
- User value: Nothing visible — an edit that would wipe a wrapper an agent or another device set goes red before it ships.
- Done when: a test per gap above, each named with its rule.
