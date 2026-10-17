# TODO-093 — (fullstack) — The public build fetches prices, off by default, for personal use

From gh289 (owner, 2026-10-17), which this entry closes when it ships. The public build has no price source (ADR-021, MKT-210): every price is typed, likely the first reason a new user gives up. Other open-source trackers let each user fetch Yahoo Finance prices from their own machine for personal use. A stopgap for the launch (owner, 2026-10-17); the licensed feed of TODO-039 (gh290) stays the target, so Yahoo comes in behind the `PriceProvider` seam (ADR-020) as one provider of a list.

**User value:** A new user gets their prices filled automatically with one switch, while a licensed feed is being set up.
**Done when:** The public build plugs in a list of price providers, one active, chosen per computer (Off by default, "Yahoo Finance — personal use"); the switch, in the first run (TODO-092) and the settings, turns on only after a checkbox acknowledging the notice (an unofficial source, used from this computer, subject to its terms, that may stop); with it off no request leaves the computer, with it on prices are fetched as in the private build; a failed or rate-limited fetch leaves the last price, judged old as TODO-085 says; a test switches the provider and checks only the chosen one is called; a new ADR supersedes ADR-021 and amends ADR-020, and MKT-210 and its guard test change with it.
**Design:** none
**Open questions:**

- [x] The notice: an explicit acknowledgement? — A checkbox, ticked once, before the switch turns on. (owner, 2026-10-17)
- [x] Per portfolio (synced) or per computer? — Per computer. (owner, 2026-10-17)
