# 2026-10-18 — DEBT-141 — A quantity an agent sends as a small JSON number is refused

- Found by: the owner, gh274 (2026-10-17), the half kept for the MVP; read on 2026-10-18 in `figure()` (`src-tauri/src/agent_connection/tools.rs`)
- Where: `figure()` turns a JSON number into text with `Number::to_string`, which writes a number below 0.00001 in exponent form (`2e-6`); `decimal_to_micro` then refuses it (AGT-048)
- Severity: 🟡
- Size: S — one pull request: the agent's figure parser and its test.
- Observation: `0.000024` passes and `0.000002` is refused with `quantity is not a decimal with a dot and at most six decimals: "2e-6"`, although both have six decimals. Monthly staking rewards of one to five millionths of a BTC cannot be recorded one by one. The precision beyond six decimals is DEBT-137.
- User value: An agent records a crypto quantity of a few millionths as the statement shows it.
- Done when: a figure sent as a JSON number in exponent form with at most six decimals (`2e-6`, `1.5e-5`) is read as its decimal value, one with more is refused as today, and a test of the agent's tools holds both.
