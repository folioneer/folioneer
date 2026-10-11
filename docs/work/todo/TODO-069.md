# TODO-069 — (fullstack) — A journal of changes that can be rewound (deferred)

Raised by the owner on 2026-10-11, with TODO-059. A price keeps no earlier value: once replaced, what was there is gone, and "Remove everything it recorded" can delete what an agent session recorded but cannot bring back what it replaced. A journal keeps what each change replaced, so that the portfolio can be taken back to an earlier point when needed.

Not urgent. To be written when scheduled: what the journal holds (prices only, or every record), how far back, and what "rewind" means beside multi-device sync, which already keeps a log of changes per device (ADR-019).

**User value:** The owner takes the portfolio back to how it stood before a change that turned out wrong — an agent's session, an import, a mistaken edit.
**Done when:** to be written with the entry's specification.
**Design:** needed
**Open questions:**

- [ ] What is journaled: prices only, or every record the owner, an agent, a fetch or a sync changes?
- [ ] What is rewound: one change, everything a session or a command did, or the whole portfolio to a moment?
- [ ] On several computers, is a rewind a change like any other that sync carries, or local to the computer that made it?
