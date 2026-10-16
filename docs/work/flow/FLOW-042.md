# FLOW-042 — The local harness never builds the tests without the `app` feature

- Kind: speed
- Observed: in the 0.11.0 batch #254 went round again because a test reached
  `agent_connection::api`, a module compiled only with the `app` feature, which the
  Windows job of Quality builds without (`cargo test --lib --no-default-features
agent_connection`). The local harness passed: it builds with the default features only.
- Proposal: `scripts/check.py`'s backend checks also compile the library's tests
  without default features when a file of `agent_connection/` changed — `cargo test
--lib --no-default-features --no-run` — as CI's Windows job does.
- Costs: one more build of the tests, about a minute, on the pull requests that touch
  the agent connection. Protects: a CI round, and a Windows-only failure found on Linux.
