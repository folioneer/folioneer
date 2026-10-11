# TODO-077 — (backend) — The command line records a split, free shares, interest and a management fee (deferred)

Moved out of TODO-071 and TODO-072 by the owner on 2026-10-11: an interface gets a recording in this order — the window, the agent connection, the command line. Once an agent records a kind, the recording is in the recorder the command line shares; what is left for the command line is its command, its page of help and the line it prints.

**User value:** A script records the same kinds an agent does, from a terminal.
**Done when:** for each kind an agent records and the command line does not — a split and free shares after TODO-071, interest and a management fee after TODO-072 — a command records it through the shared recorder, with its page of help, its printed line and its JSON; the README lists it.
**Design:** none
**Open questions:**

- [ ] The names of the commands (`holding split`, `holding free-shares`, …): as the owner types them.
