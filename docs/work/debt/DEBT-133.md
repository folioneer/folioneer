# 2026-10-17 — DEBT-133 — Two clauses of the agent connection need a channel a test can hold

- Found by: the main agent, closing DEBT-130 (2026-10-17): the two clauses below could not get a test that fails when they break, with the live bridge and the application the suite drives
- Where: `src-tauri/src/agent_connection/{bridge,server,end_to_end_tests}.rs`
- Severity: 🔵
- Size: S — one pull request: a test seam on the application's end of the channel, two tests.
- Observation: **AGT-085, the yes and the cancel crossing**: a yes read by the application before the bridge reads the cancel opens a session that ends as the connection closes. Which one is read first depends on scheduling; no test can order them. **AGT-086, a write already started**: an agent that goes away mid-call leaves a write already started to run to its end, and the window is told. A test that closes its input right after a recording call finds no write — the call is dropped before it starts, which the rule allows — and has no way to hold a write mid-way
- User value: None directly.
- Done when: the application's end of the channel can be held at a chosen step in a test (after reading a yes, after a write starts), and each clause has a test that fails when it is broken.
