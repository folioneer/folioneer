# 2026-10-11 — DEBT-119 — An asset an agent added says nowhere that an agent added it

- Found by: the owner, on 2026-10-11, when TODO-063 was queued: the session dialog will say how many assets a session added, and nothing keeps it once the session ends
- Where: `docs/spec/agent-connection.md` (AGT-045, the mark of a transaction; AGT-062, the source of a price); `docs/work/todo/TODO-070.md` (every change says who made it, deferred)
- Severity: 🔵
- Observation: a transaction an agent recorded carries a mark and a price a source; an asset an agent adds (TODO-063) carries neither, by the owner's choice for that entry. The same is true of any later change an agent makes that is neither a transaction nor a price
- User value: The owner sees which assets an agent added, after the session as during it.
- Done when: an asset an agent added says so where the owner reads assets, in the way a price or a transaction does — or TODO-070 ships and covers it.
