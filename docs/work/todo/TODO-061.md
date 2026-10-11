# TODO-061 — (fullstack) — An agent asks before removing a transaction the owner typed

Split from TODO-059 on 2026-10-10; after TODO-060. For a transaction the owner typed, the agent may only ask: the window shows the transaction in full (date, asset, quantity, amount) and what would become of it, refusing is the default action, the session's grant never covers it, and there is no "allow all". The position typed at the start of this year is the owner's to remove, in the window or by allowing the agent's request, once the past is rebuilt. Narrowed on 2026-10-11 with the owner to the removal, which is that use; asking to change one is TODO-080.

The mock-up of the ask is `screenshots/design/061-*-ask.png`, validated by the owner with TODO-051 on 2026-10-09.

**Size:** L — two pull requests: the request and its rules, then the dialog with its screenshots.
**User value:** The owner lets an agent replace a typed position by the rebuilt history, one transaction at a time, seeing exactly what goes.
**Done when:** a removal of a transaction the owner typed is asked in the window, shown in full, refused by default, one at a time; a refusal gives the agent nothing; nothing of an answer is remembered for the next ask.
**Design:** validated
**Open questions:** none
