# 2026-10-16 — DEBT-132 — What /prune found in what v0.11.0 changed

- Found by: `/prune` on the production code v0.11.0 changed since v0.10.0, after the release (2026-10-16); it only read
- Where: `src/features/shell/agent_connection/AgentConnectionDialog.tsx`, `src/features/shell/agent_connection/RemovalRequestDialog.tsx`
- Severity: 🔵
- Size: S — one pull request: one hook, two dialogs, no visual change.
- Observation: the two dialogs each define the same effect that keeps their approving action inactive for `ALLOW_AFTER_MS` after they appear (AGT-032, AGT-083); the lines are covered in both. The focus trap they share is already one hook, `useKeepFocusOnActions`. Skipped as partly covered: the bridge's line reading while a call waits (`rest_of_line`, `take_line_part`, `whole_line` in `bridge.rs`) repeats `wire::read_line`, but its "line too long" branch is not hit
- User value: None directly.
- Done when: one hook beside `useKeepFocusOnActions` keeps the action inactive, used by both dialogs, and their tests stay as they are.
