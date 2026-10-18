# 2026-10-18 — DEBT-146 — Spec text lags the code in market price and account details

- Found by: `spec-checker` at the hand-over of the 2026-10-18 batch, on main at 7938e09
- Where: `docs/spec/market-price.md`, `docs/spec/scheduled-price-fetch.md`, `docs/spec/account-details.md`
- Severity: 🔵
- Size: S — docs only.
- Observation: An asset with no derivable symbol never enters the scope (`scope.rs:51-55`, MKT-111, SPF-040), yet MKT-119, MKT-171, MKT-220 and SPF-041 say it is counted as skipped and listed. MKT-030/140 say "Updated today / Nd ago"; the window says "0d / Nd". MKT-020 says the asset is not modifiable; MKT-011 and the code make it a choice. MKT-037 names an `ARCHITECTURE.md` event table that no longer exists. ACD-037 shows the skeleton on every re-fetch; the code only on the first load (F29). ACD-048 lists three columns of the closed table; it has five.
- User value: Nothing visible — the specs say what the screens do.
- Done when: each rule above states what the code does (the non-derivable symbol outside the count), `spec-reviewer` clean on the three files.
