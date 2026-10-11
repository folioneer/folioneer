# TODO-070 — (fullstack) — Every change says who made it (deferred)

Raised by the owner on 2026-10-11, at the hand-over of TODO-059. The portfolio answers "who made this?" twice, in two ways. A transaction an agent recorded carries a mark: the agent and its session, kept in a table of its own on the computer that recorded it (AGT-045). A price carries a source: "Agent" says an agent wrote it and not which session, and the same field also says where a figure comes from ("Yahoo Finance"). Neither is general: an asset an agent adds (TODO-063) would need a third. So "Remove everything it recorded" removes a session's transactions and leaves its prices, and nothing lists what a session, a command or a fetch changed.

What is missing is the change itself with who made it: every write — a creation, an update, a removal, of a transaction, a price, an asset — recorded with its author: the owner in the window, the owner on the command line, an agent in a session, the application on its own (a fetch, a generated fee), another of the owner's computers. "Recorded by", the source of a price read as "who wrote it", the removal of a session's work and the journal of TODO-069 would then each read the same record.

Sync already records a change for every write of a synced record (the confirmed term "Change": one recorded modification of one record, made on one device), with an origin that is "user" or "application". Whether that log is the place — whether it is written when sync is off, and what it would take to carry an author and the value replaced — is to be read when this is scheduled.

Until this ships: a recorded price stays as TODO-059 left it, and the removal takes transactions only.

**User value:** The owner sees who made any change — himself, an agent's session, a fetch — and undoes everything one session did, prices included.
**Done when:** to be written with the entry's specification; at least: every change of a record carries who made it; "Recorded by" and the removal of a session's recordings read it, for a transaction and for a price.
**Design:** needed
**Open questions:**

- [ ] The words. "Change" is the confirmed term for one recorded modification of one record. Is that the word here too, rather than "operation" — which in French reads as a bank operation, and in the code names whether a change creates, updates or removes? And what names who made it: "author", or another word?
- [ ] Who are the authors: the owner in the window, the owner on the command line, an agent with its session, the application on its own, another computer — and is the command line told apart from the window?
- [ ] Is who made a change part of what sync carries, so that every computer shows it? It then replaces the mark that TODO-062 would send, and that entry folds into this one.
- [ ] Does a price keep its source as where the figure comes from (typed, fetched from a provider), "Agent" leaving the source for the author?
