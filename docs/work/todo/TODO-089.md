# TODO-089 — (fullstack) — A return of capital lowers the cost of a holding and is no income

From gh241 (owner, 2026-10-12), one of its two parts; the issue closes when this entry and TODO-073 have both shipped. Rebuilding years of an account from statements met events no transaction type models. The issue names four; read on 2026-10-13, they come to this.

**A return of capital** is new: a company pays back part of its capital — cash comes in, the quantity stays, and the cost of the holding goes down by the amount; it is no income. Recorded as a dividend today, it overstates income and leaves the cost too high. Example: 100 units costing 2 000, a return of 0.30 a unit: cash +30, cost 1 970. This entry is that type.

**Cash in lieu of a fraction** needs no type of its own where the fraction exists: quantities are kept to six decimals, so a split or free shares leave the fraction in the holding, and selling that fraction for the cash received is a sale — a small realised gain against its share of the cost, no income. It is recorded as a dividend today for want of knowing that. Whether a broker's rounding should be recorded in one step with the event is a question of the share exchange (TODO-073), which already asks it.

**A distribution of another company's shares** — a spin-off — is an exchange in part: an asset comes into the account with a cost taken from another holding. It goes to TODO-073, with its own question there.

**A dividend paid in shares with a cash top-up** is a dividend and a purchase, as the issue supposes.

**User value:** A history rebuilt from statements gives the cost basis and the realised gains the broker gives, and income figures that hold income only.
**Done when:** a return of capital exists as a transaction type in the core, with its effect on the cost and on the cash specified and tested, corrected and deleted like the others; it is recorded in the window, then by an agent and on the command line, in the order the owner chooses; the README or the tools' descriptions say how a fraction paid in cash and a dividend paid in shares are recorded.
**Design:** none
**Open questions:**

- [x] A return of capital larger than what the holding still costs: the excess is a realised gain, or the recording is refused? — The cost stops at 0 and the excess is its own figure, "return of capital beyond cost", counted as realised in the performance meanwhile; each country's year-end report counts it as its law says (a gain in the United States, income in Spain, unsettled in Italy). Japan's return of capital (a deemed dividend and a deemed sale by a net-asset ratio) is another mechanism, not covered. (owner, 2026-10-18)
- [x] The amount is typed per unit or in all — and in the asset's currency with a rate, as a dividend is? — As a dividend is: the total received, in the asset's currency with a rate when it differs. (owner, 2026-10-18)
- [x] Its name in both languages ("Return of capital" / "Remboursement de capital"?), and its place in the window: the holding's row, as a dividend is recorded. (A screen changes: a design to validate.) — "Return of capital" / "Remboursement de capital" / "Kapitalrückzahlung", recorded from the holding's row beside the dividend. (owner, 2026-10-18)
- [x] The order of the interfaces: the window first, then the agent connection, then the command line, as for a split (owner, 2026-10-11)? — The window, then an agent, then the command line (TODO-077). (owner, 2026-10-18)
