# TODO-080 — (fullstack) — An agent asks before changing a transaction the owner typed

Split from TODO-061 on 2026-10-11 with the owner; after it. The ask of TODO-061 removes a typed transaction; this one changes it — a date, a quantity, an amount — through the same dialog, which then shows the transaction as it is and as it would become.

**Size:** M — one pull request once TODO-061's dialog exists.
**User value:** The owner lets an agent correct a transaction he typed, seeing exactly what changes.
**Done when:** a change of a transaction the owner typed is asked in the window, shown as it is and as it would become, refused by default, one at a time; a refusal gives the agent nothing; nothing of an answer is remembered for the next ask.
**Design:** none
**Open questions:**

- [x] The mock-up validated for TODO-061 (`screenshots/design/061-*-ask.png`) shows the ask "with what would become of it": does it stand for a change as it is, or does a change — two states of one transaction side by side — need its own mock-up? — Its own mock-up: two states of one transaction side by side is another layout than one transaction leaving. It is drawn once TODO-061's dialog exists; the entry waits for the queue after it (owner, 2026-10-12).
