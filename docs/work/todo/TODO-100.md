# TODO-100 — (fullstack) — Year-end figures for the tax return, France and Germany first

From gh299 (owner, 2026-10-17), which this entry closes when it ships. What makes a local investor adopt a tracker is the evening it saves at tax time, above all with a foreign broker that issues no national statement. France and Germany first (owner, 2026-10-17); Spain, Italy and Poland follow with their countries (gh305). No tax is computed, no advice given. Needs the wrappers (TODO-091), FIFO (TODO-099) and, for Germany, each fund's type.

**User value:** At tax time the user gets the figures their country asks for, from all their brokers at once.
**Done when:** A year-end report per account and country — France: realised gains (weighted average), dividends and interest by account, crypto disposals for form 2086, foreign accounts for form 3916; Germany: realised gains by FIFO split shares and other, distributions, each accumulating fund's Vorabpauschale, the Teilfreistellung, the Sparer-Pauschbetrag per broker — computed by the backend, each line saying the rule it follows, shown and exported as CSV and PDF in English, French and German; a country is a module added without touching the others; each country tested on a fixture portfolio with hand-checked values; every page says it computes no tax and is not advice.
**Design:** none
**Open questions:**

- [x] Which year-end value? — The last price on or before 31 December, its date shown on the line. (owner, 2026-10-17)
