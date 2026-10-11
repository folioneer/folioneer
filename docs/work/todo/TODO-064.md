# TODO-064 — (fullstack) — One table says what the window, the command line and an agent can each do

Raised on 2026-10-10. The three interfaces share their rules — one recorder, one core — and not their capabilities: the window does everything; the command line records three transactions, adds an asset and lists accounts and assets; an agent reads the summary and the holdings, which the command line cannot, and cannot add an asset, which the command line can. Nothing says which gap is a choice and which an accident.

One document lists each capability and which interface offers it. A check fails when a tool or a command is added without its line. The same table ships with the application and is shown in it, as the page that says what this version can do through each interface (owner, 2026-10-10).

**User value:** The owner reads in the application what an agent and the command line can do in this version, and a new capability never appears in one interface by accident.
**Done when:** the table exists in the docs, one line per capability, one column per interface; a check fails on a tool or a command with no line, seen red; the application shows the table on a page of its own, in both languages, from the same source as the docs.
**Design:** none
**Open questions:**

- [ ] Where does the page live in the application: under Settings beside "Allow agents to connect", or in About?
- [ ] Should the command line read what an agent reads (the portfolio summary, the holdings of an account)? The table makes the gap visible; this decides whether to close it.
