# 2026-10-18 — DEBT-142 — The outdated-price thresholds are fixed in the code

- Found by: the owner, answering TODO-085 in the batch's opening block (2026-10-18): fixed defaults first, adjustable once a synced place for portfolio settings exists (TODO-032)
- Where: `outdated_after` in `src-tauri/src/context/asset/domain/outdated_price.rs` (MKT-205)
- Severity: 🔵
- Size: M — a synced portfolio setting, the rule reading it, and a settings section.
- Observation: a price is outdated past 5 business days for a listed or crypto asset and 10 for a fund, never for a custom asset, cash or real estate. The owner wants these defaults adjustable per asset class; they are constants until TODO-032 ships the synced portfolio settings.
- User value: A user whose funds publish monthly, or who wants a tighter watch on shares, sets the thresholds that fit their portfolio.
- Done when: the thresholds per asset class are a portfolio setting, synced, defaulting to today's values; MKT-205 reads them; the settings page edits them in English and French; the golden portfolio does not move.
