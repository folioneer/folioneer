# TODO-061 — (fullstack) — An agent asks before removing a transaction the owner typed

Split from TODO-059 on 2026-10-10; after TODO-060. For a transaction the owner typed, the agent may only ask: the window shows the transaction in full (date, asset, quantity, amount) and what would become of it, refusing is the default action, the session's grant never covers it, and there is no "allow all". The position typed at the start of this year is the owner's to remove, in the window or by allowing the agent's request, once the past is rebuilt. Narrowed on 2026-10-11 with the owner to the removal, which is that use; asking to change one is TODO-080.

The mock-up of the ask is `screenshots/design/061-*-ask.png`, validated by the owner with TODO-051 on 2026-10-09.

**Size:** L — two pull requests: the request and its rules, then the dialog with its screenshots.
**User value:** The owner lets an agent replace a typed position by the rebuilt history, one transaction at a time, seeing exactly what goes.
**Done when:** a removal of a transaction the owner typed is asked in the window, shown in full, refused by default, one at a time; a refusal gives the agent nothing; nothing of an answer is remembered for the next ask.
**Design:** validated
**Open questions:**

- [x] The connection dialog says an agent corrects or cancels "never a transaction you typed": what does it say once an agent may ask to remove one? — "It can correct or cancel only what it recorded in this session; for a transaction you typed it can only ask you, one at a time — and it never reads a file, a setting or your sync folder." ("Il ne peut corriger ou annuler que ce qu'il a enregistré pendant cette session ; pour une transaction que vous avez saisie, il ne peut que vous demander, une à la fois — et ne lit jamais un fichier, un réglage ou votre dossier de synchronisation.") (owner, 2026-10-12).
- [x] The term for an agent's ask to remove a typed transaction, and the tool it calls? — "Removal request" ("Demande de suppression"), added to `docs/ubiquitous-language.md`; the tool is `request_removal` (owner, 2026-10-14).
