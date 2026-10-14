# TODO-078 — (backend) — A call the agent gave up on withdraws its connection request

Split from TODO-065 on 2026-10-11 with the owner. A call the agent gave up on leaves its connection dialog on screen until the agent's process ends: the owner is asked about a connection nobody waits for.

From TODO-061's review (2026-10-14): a removal request (AGT-080) waits inside a tool call, and an agent that leaves while it waits leaves the request on screen until the owner answers it; a call nobody waits for withdraws it the same way.

**Size:** M — one pull request: the bridge, the gate and a rule of the agent's spec.
**User value:** A dialog never asks about a connection nobody waits for.
**Done when:** a cancelled call (`notifications/cancelled`) withdraws its connection request, and the dialog leaves the window; a cancelled call that was already allowed changes nothing of its session.
**Design:** none
**Open questions:** none
