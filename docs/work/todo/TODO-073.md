# TODO-073 — (fullstack) — A share exchange: one asset given for another, without a gain or a loss (deferred)

From gh209, refined with the owner on 2026-10-11. A merger or an acquisition exchanges the shares of one asset for another's; recorded as a sale and a purchase it shows a gain or a loss that was never realised and breaks the cost basis. The application does not model it: no transaction kind, no rule (the roadmap lists "Merger / acquisition" as planned). It becomes more urgent with time, since the core lacks it altogether.

In this order (owner, 2026-10-11): the model first — the rules and the core; then the interfaces, the window before the agent connection, the agent connection before the command line. Each is its own pull request.

**User value:** A holding that went through a merger keeps its cost basis and shows no gain or loss it never had.
**Done when:** to be written with its specification; at least: an exchange moves the cost basis from the asset given to the asset received and realises no gain or loss, except for a cash part; it is recorded, corrected and removed in the window; then by an agent; then on the command line.
**Design:** needed
**Open questions:**

- [ ] A fraction of a share the exchange leaves: kept as a fraction, or paid in cash?
- [ ] The cash part of an exchange (a balancing amount): a gain realised on that part alone, or a reduction of the cost basis?
- [ ] One transaction on two assets, or two linked transactions — and how does the journal of each asset show it?
- [ ] A distribution of another company's shares (a spin-off, gh241): the new asset's cost is taken from the holding that gave it — by a ratio the owner types, by the broker's ratio — or is zero? And is it this entry's exchange with nothing given up, or a transaction of its own? gh241 closes when this entry and TODO-089 have both shipped.
- [ ] A fraction the event leaves and the broker pays in cash (gh241): recorded with the event in one step, or left as a sale of the fraction, which works today?
