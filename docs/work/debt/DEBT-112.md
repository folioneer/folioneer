# 2026-10-11 — DEBT-112 — A purchase's default fees and rate are decided in each interface, and a note keeps its control characters

- Found by: `reviewer-arch` and `reviewer-security` on TODO-059's first pull request (2026-10-11); both read again in the code by the main agent
- Where: `trade` in `src-tauri/src/agent_connection/tools.rs` (`unwrap_or(0)`, `unwrap_or(1_000_000)`) and `read` in `src-tauri/src/command_line/args.rs` (`amount_or("--fees", 0)`, `amount_or("--rate", MICRO)`); `note` in `src-tauri/src/agent_connection/tools.rs`
- Severity: 🔵
- Observation: the fees of 0 and the rate of 1 a purchase or a sale takes when none is given (CLI-014) are written once in the agent connection and once in the command line, so the two can drift (B47); the dividend added by TODO-059 takes its default in the recorder. A note an agent sends is bounded to 500 characters and may hold control characters: the window escapes them when it shows a note, and nothing else prints one today.
- User value: None directly — the command line and an agent cannot come to disagree on what a recording without fees or rate means.
- Done when: `Trade` carries optional fees and rate and the recorder gives the defaults, with a test through both interfaces; a note with a control character other than a line break is refused by the recorder, or the rule says it is kept.
