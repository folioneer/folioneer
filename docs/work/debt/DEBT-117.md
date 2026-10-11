# 2026-10-11 — DEBT-117 — Five rules older than TODO-059 that no test holds through their interface

- Found by: `spec-checker` on `docs/spec/command-line.md` and `docs/spec/multi-device-sync.md`, at the hand-over of the batch (2026-10-11); it only read. Split from DEBT-113 on 2026-10-11. Nothing below was read again by the main agent
- Where: `src-tauri/src/command_line/headless.rs` (tests); `src-tauri/tests/sync_two_devices.rs`
- Severity: 🔵
- Size: M — one pull request: backend tests.
- Observation: each is implemented, and tested in the core or not at all. CLI-017: an archived asset bought through `holding buy` stays archived (tested in the core only). CLI-031: a command's write leaves a change for sync. CLI-032: a user without a data folder is refused. CLI-033: a command reaches no network and listens to nothing. SYN-038: a change written in data format version 1 or 2, applied by this build.
- User value: None directly.
- Done when: each rule above has a test through the command line, or through a segment written in the older format, that fails when the rule is broken.
