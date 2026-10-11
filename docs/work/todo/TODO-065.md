# TODO-065 — (backend) — An agent client knows a read from a recording

Raised on 2026-10-10 by an audit of the agent connection against the protocol's practices (DEBT-089). No tool says whether it only reads or changes something, so an agent client cannot ask its user before a recording and skip the question for a read. Split on 2026-10-11 with the owner: the notice in the window is TODO-079, the cancelled call TODO-078.

**Size:** M — one pull request: the tools' definitions and a rule of the agent's spec.
**User value:** Claude asks before recording and not before reading.
**Done when:** each tool carries a title and says whether it only reads (`readOnlyHint`); a test holds that every recording says it does not, and that every tool the program lists says one or the other.
**Design:** none
**Open questions:** none
