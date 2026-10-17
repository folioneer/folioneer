# TODO-080 — (fullstack) — An agent asks before changing a transaction the owner typed

Split from TODO-061 on 2026-10-11 with the owner; after it. The ask of TODO-061 removes a typed transaction; this one changes it — a date, a quantity, an amount — through the same dialog, which then shows the transaction as it is and as it would become.

**Size:** M — one pull request once TODO-061's dialog exists.
**User value:** The owner lets an agent correct a transaction he typed, seeing exactly what changes.
**Done when:** a change of a transaction the owner typed is asked in the window, shown as it is and as it would become, refused by default, one at a time; a refusal gives the agent nothing; nothing of an answer is remembered for the next ask.
**Design:** validated (`screenshots/design/080-*.png`)
**Open questions:**

- [x] The mock-up validated for TODO-061 (`screenshots/design/061-*-ask.png`) shows the ask "with what would become of it": does it stand for a change as it is, or does a change — two states of one transaction side by side — need its own mock-up? — Its own mock-up: two states of one transaction side by side is another layout than one transaction leaving. It is drawn once TODO-061's dialog exists; the entry waits for the queue after it (owner, 2026-10-12).
- [x] Its mock-up, now that TODO-061's dialog exists: drawn when? — In the next batch's opening block, and validated there (owner, 2026-10-17).
- [x] Which typed transactions may an agent ask to change? — Every kind, through the window's own edit: the date, the figures and the note its form lets the owner change — the reach of the removal request (owner, 2026-10-17). The Size may grow past M; split at the PR if it crosses § 11.
- [x] The mock-up (`screenshots/design/080-*.png`): two columns, "As it is" and "As it would become", the changed values struck through and in the accent colour; "Refuse" filled and in focus; "Change this transaction" inactive for a moment — validated (owner, 2026-10-17).
- [x] Its term? — "Change request", the tool `request_change`, in French "Demande de modification"; in `docs/ubiquitous-language.md` (owner, 2026-10-17).
