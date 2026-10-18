# TODO-112 — (frontend) — An outdated price and a failed fetch say so in one short chip

Asked by the owner (2026-10-18), on TODO-085's French wordings: the holding row is too wordy. Today an outdated price shows a chip "Outdated price · 7d" ("Prix ancien · 7j") and, when this device's last fetch did not update the price, a visible line under the price ("Non mis à jour : aucun prix du fournisseur") whose date shows only on hover (`HoldingRow.tsx:437-490`, MKT-207, MKT-222).

**User value:** A holding row says in one short chip that its price is old, and why on hover, instead of two lines of text.
**Done when:** the chip reads "Outdated · 7d" / "Périmé · 7j"; the visible failure line is gone; the reason and the failed fetch's date are the chip's hint on hover — "No price found (tried on 18 Oct)" / "Aucun prix trouvé (essai du 18 oct.)", "Provider unreachable (tried on 18 Oct)" / "Fournisseur injoignable (essai du 18 oct.)", "Price not saved (tried on 18 Oct)" / "Prix non enregistré (essai du 18 oct.)"; a price not yet outdated whose last fetch failed shows a small warning icon carrying the same hint; MKT-207 and MKT-222 and the vocabulary entry say so; tests and visual proof in both themes.
**Size:** S
**Design:** validated — no mock-up needed (owner, 2026-10-18); the wording above is the design.
**Open questions:** none
