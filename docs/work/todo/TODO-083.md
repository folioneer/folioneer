# TODO-083 — (backend) — What an agent adds or records shows in the window at once

From gh247 (owner, 2026-10-12), which this entry closes when it ships: an asset an agent added with `add_asset` was in `list_assets` and in the price dialog, and the assets table showed it only minutes later, after other actions. Seen once on the installed build, not reproduced.

Read on 2026-10-13, not run: the event bus keeps one event — it is a `watch` channel, and the forwarder to the window reads what the channel holds when it wakes (`core/event_bus/bus.rs`, `app.rs`). A tool call publishes twice with nothing between: the write's own event from the service (`AssetUpdated` for an asset, `TransactionUpdated` for a recording), then `AgentConnectionChanged` once the call is counted (`server.rs`, `connections.notify()` right after `tools.run`). When the forwarder has not run between the two, the window is told the second only. `docs/backend-rules.md` B45 says an action publishes exactly one event, for this reason. It would explain an asset that shows late, and it holds for every write an agent makes, not for assets alone.

**User value:** What an agent adds or records shows in the window right away, as what the owner types does, so the owner checks it while the agent is still working.
**Done when:** a test makes a write through the agent connection — an asset, a transaction, a price — with a listener on what the window is told, and both the write's event and the session's change reach it, every time; the same is held for a call that follows another at once; B45 and the agent's spec (AGT-036) say what a tool call publishes.
**Design:** none
**Open questions:** none
