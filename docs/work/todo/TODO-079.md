# TODO-079 — (frontend) — The window says when an agent records

Split from TODO-065 on 2026-10-11 with the owner. A recording made by an agent shows only if the journal of its account is open; on any other view nothing says it happened. The window already reads, for each session, what it recorded last (AGT-052): the notice is said from that.

**Size:** M — one pull request: the window, its strings in two languages, its screenshots; the sentences are the owner's to validate.
**User value:** The owner sees each recording an agent makes as it happens, whatever view is open.
**Done when:** the window shows a notice naming the agent and what it recorded, on every view; a recording that is cancelled or corrected is not announced as a new one.
**Design:** none
**Open questions:**

- [x] The notice on every view: the window's existing snackbar, or a new element? — The existing snackbar, one message per recording, a new one replacing the one before; no new element and no layout change, so no mock-up. Its sentences, in English and in French, are shown to the owner before they are coded (owner, 2026-10-11).
