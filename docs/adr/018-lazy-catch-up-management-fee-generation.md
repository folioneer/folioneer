# ADR 018 — Lazy Catch-Up Generation for Recurring Management Fees

**Date**: 2026-06-30
**Status**: Accepted

## Context

A management fee schedule (FEE-030) charges a held asset a recurring percentage of its quantity at a fixed cadence (monthly / quarterly / annually). Each due period must materialize a `ManagementFee` transaction (FEE-040–047) that reduces the holding's quantity, so the deduction is a real, visible ledger entry — not a figure derived only at display time. The app is a desktop Tauri application with no always-on server process: it is open only while the user runs it, and may be closed for arbitrarily long stretches.

## Decision

Generate due deductions by **lazy catch-up on app start**: a single shell-mounted startup hook fires one catch-up command on mount, which walks every active schedule, replays from its persisted period cursor up to today, and records one deduction per due period boundary (advancing the cursor as it goes, so a period is never double-charged). Alternatives considered: (a) a **background scheduler / OS timer** that fires on the real calendar date — rejected because a desktop app has no resident process to host it, and a missed window (app closed) still needs catch-up logic, so the scheduler adds moving parts without removing the cursor-replay code; (b) a **fully derived, never-materialized** fee computed on read — rejected because the product requirement is that fees appear as concrete ledger transactions affecting cost-basis replay (FEE-022/023), which a read-time-only figure cannot do. Catch-up delegates to the same recording path used for one-off deductions, inheriting its sequential reduction and oversell guard for free, and mirrors the project's replay-everywhere model.

## Consequences

- **Pros**: no resident process or platform timer; survives the app being closed for months (the gap backfills on next open); the per-schedule date cursor makes generation idempotent; reuses the one-off record path, so there is a single deduction code path to test; deductions are real transactions, consistent with cost-basis replay and the Management Fees aggregation (FEE-052/053).
- **Cons**: fees materialize when the user next opens the app, not on the calendar date — an account never reopened accrues nothing until reopened (acceptable: an unused app has no live valuation to act on anyway); catch-up cost scales with the number of missed periods × active schedules on a cold open after a long gap (bounded and small at personal scale); a generation failure is best-effort and surfaced via a snackbar (F27) rather than blocking startup.
