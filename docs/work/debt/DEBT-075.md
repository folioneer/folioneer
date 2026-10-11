# 2026-10-04 — DEBT-075 — A split factor has no upper bound, and the replay's quantity can wrap

- Found by: reviewer-security and reviewer-backend on DEBT-071
- Where: `Transaction::split` in `src-tauri/src/context/account/domain/transaction.rs` (accepts any positive factor); the split arms of the replay in `src-tauri/src/context/account/domain/account.rs` (`total_quantity as i64`)
- Severity: 🔵
- Observation: the replay multiplies the quantity by the factor in 128 bits and casts the result back to 64 bits without a check. A factor large enough (far beyond any real split) makes the cast wrap: recording and the split check both then show a quantity that is wrong, or refuse a split for the wrong reason.
- User value: None in practice — no real split comes near the bound; an absurd factor typed by mistake is refused with a clear reason.
- Done when: a factor whose rescale cannot be stored is refused when the split is recorded and when it is checked, with one error code, shown by a test on each path.
