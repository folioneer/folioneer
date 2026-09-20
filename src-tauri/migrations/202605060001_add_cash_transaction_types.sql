-- CSH-022 / CSH-032 — Documentation-only migration.
--
-- Cash Tracking introduces two new TransactionType discriminants on the
-- Rust side: `Deposit` and `Withdrawal`. The `transactions.transaction_type`
-- column is already TEXT NOT NULL DEFAULT 'Purchase' (see
-- 202604120002_create_transactions.sql), so no schema change is required —
-- the new variants are valid string values out of the box.
--
-- This file exists so the migration history records the moment new variants
-- entered the wire format, and so `cargo sqlx prepare` is forced to refresh
-- on a fresh checkout. The body is intentionally a no-op SELECT (sqlx
-- migrations require at least one statement).
--
-- Cash Asset and Cash Category records are seeded lazily at runtime by
-- `ensure_cash_asset(currency)` (CSH-010 / CSH-011 / CSH-017) — not via
-- migration — because seeding is currency-driven, not migration-driven.

SELECT 1;
