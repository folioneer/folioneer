# 2026-10-18 — DEBT-152 — What a join does with a device's own reference currency is undecided

- Found by: the spec review of TODO-032 part 1 (2026-10-18); carried from TODO-032's last open question when the entry closed
- Where: `docs/spec/multi-device-sync.md` SYN-014 / SYN-083 and its Open Questions; `docs/spec/sync-conflict-resolution.md` CFR-034 / CFR-060; `docs/spec/fx-rate.md` FXR-120–125
- Severity: 🟡
- Size: S — two rules, a two-device test.
- Observation: since TODO-032 a device can choose a reference currency before it joins a shared portfolio (and TODO-092's first run will ask one). Today the joining device's unranked setting yields to the shared portfolio's ranked one when the history is applied — read in the code, not stated by a rule, and not tested. Whether that is right, and whether two devices that chose apart should be told (a natural-key collision, CFR-060), is the owner's decision.
- User value: A device joining a shared portfolio shows it in the currency the user expects, and nothing about a choice is lost without the user knowing.
- Done when: the owner's answer is written as rules in SYN-014/SYN-083 (and CFR-034/060 if a notice is wanted), each with a two-device test.
- **Open questions:**
  - [ ] A device joining with its own reference currency: does the shared portfolio's choice replace it, or does it block the join as user data? And two devices that chose apart: is it reported as a collision, or does the later choice win silently? (Recommended: the shared portfolio's choice replaces the joining device's; two choices made apart, the later wins silently — a setting is not user data that can be lost.)
