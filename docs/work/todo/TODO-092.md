# TODO-092 — (fullstack) — A first run sets the country, the currency and the accounts

From gh284 (owner, 2026-10-17), which this entry closes when it ships. A new user opens an empty window: no account, no hint of what to do first. Each step exists or is coming — accounts, the reference currency (TODO-032), the import (TODO-025), the agent connection (TODO-067) — and nothing leads from one to the next.

**User value:** A new user knows what to do in the first minutes and sees what the application gives before entering anything.
**Done when:** A portfolio with no account opens an assistant, skippable and reopenable from the settings: language, country and reference currency; the accounts with their wrapper (TODO-091); how to bring the history in (import, an agent, or by hand); a demo portfolio opened read-only, never mixed with the user's data, never synced; each step writes through the existing rules and creates nothing the user did not confirm; in every interface language, both themes.
**Design:** none
**Open questions:**

- [ ] The demo portfolio: shipped with the application, or generated from the seed the E2E suite uses?
