# 2026-09-21 — DEBT-037 — The settings capture still carries a random folder path

- Found by: the main agent (PR #8, a Markdown-only pull request, visual gate red)
- Where: `e2e/sync/sync.test.ts:114` (`mkdtempSync(join(tmpdir(), "folioneer-sync-"))`), captured in `sync-settings-{light,dark}`
- Severity: 🔵
- Observation: the sync section renders the shared folder, whose `mkdtemp` suffix differs every run, so those pixels (columns 611–651, 0.106 % of the screen) always differ between two runs of the same commit. Below the workflow's 0.3 % threshold on its own, so nothing fails today; it was a third of the budget when the fading scrollbar took the rest and pushed the total to 0.327 %. It leaves the gate that much closer to a false regression on any screen that shares it.
- User value: None — the merge gate's headroom.
- Done when: `sync-settings` is byte-identical across two runs of the same commit; the E2E sync folder carries a fixed name (the suite runs one instance, `maxInstances: 1`) or the value is not rendered into the capture.
