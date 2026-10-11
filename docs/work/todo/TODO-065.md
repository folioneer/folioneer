# TODO-065 — (fullstack) — The window says when an agent acts, and an agent client knows a read from a recording

Raised on 2026-10-10 by an audit of the agent connection against the protocol's practices (the part of DEBT-089 a user would notice). A recording made by an agent shows only if the journal is open. A call the agent gave up on leaves its connection dialog on screen until the agent's process ends. No tool says whether it only reads or changes something, so an agent client cannot ask its user before a recording and skip the question for a read.

**User value:** The owner sees each recording an agent makes as it happens, whatever view is open; a dialog never asks about a connection nobody waits for; Claude asks before recording and not before reading.
**Done when:** the window shows a notice naming the agent and what it recorded, on every view; a cancelled call (`notifications/cancelled`) withdraws its connection request; each tool carries a title and says whether it only reads (`readOnlyHint`), and a test holds that every recording says it does not.
**Design:** none
**Open questions:**

- [x] The notice on every view: the window's existing snackbar, or a new element? — The existing snackbar, one message per recording, a new one replacing the one before; no new element and no layout change, so no mock-up. Its sentences, in English and in French, are shown to the owner before they are coded (owner, 2026-10-11).
