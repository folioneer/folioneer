# TODO-062 — (backend) — "Recorded by" shows on every computer of the sync

Split from TODO-059 on 2026-10-10. The mark of an agent's recording is kept on the computer that recorded it (AGT-045): the owner's other computers show the transaction without its "Recorded by", and "Remove everything it recorded" (TODO-060) would work on one computer only. The mark becomes part of what sync carries, so the data format version moves (SYN-038).

**User value:** Whichever computer the owner opens, the journal says what an agent recorded, and removing a session's recordings removes them everywhere.
**Done when:** a mark written on one computer reaches the others with its transaction and goes with it when the transaction is removed; a computer on the previous format is told to update, as SYN-035 says; the two-device test covers a marked transaction.
**Design:** none
**Open questions:** none
