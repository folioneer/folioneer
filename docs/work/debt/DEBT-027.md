# 2026-09-15 — DEBT-027 — ADR-012 does not record the price history backfill's fill-only exception

- Found by: spec-reviewer (MKT-190–199 second pass on TODO-008)
- Where: `docs/adr/012-latest-write-wins-source-as-metadata.md` (decision 1), `docs/spec/market-price.md` MKT-192
- Severity: 🔵
- Observation: ADR-012 decision 1 says every price write upserts unconditionally; MKT-192 exempts the price history backfill, which writes only dates without a price. The owner's reading (2026-10-11): the backfill skips a date that has a price to save the writes, not to rank one source above another. ADR-017 still lists two sources where MKT-100 has three. The spec names the exception and the ADR does not, so a reader of the ADR alone misses it; the ADR status vocabulary has no "amended by" form yet (DEBT-007).
- User value: None — the decision record matches the rules.
- Done when: ADR-012, or an ADR that supersedes it, names the fill-only exception and why it is there, and adr-reviewer passes it.
