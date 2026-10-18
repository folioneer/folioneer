# 2026-10-18 — DEBT-144 — Sync's device-local data and older account changes are held by no test

- Found by: `spec-checker` at the hand-over of the 2026-10-18 batch, on main at 7938e09
- Where: `src-tauri/tests/sync_two_devices.rs`, `src-tauri/src/context/sync/`
- Severity: 🟡
- Size: S — tests, and the snapshot generator.
- Observation: SYN-038: no test applies a format-3 account change over an account that has a country and a wrapper (the merge in `apply.rs` keeps them — read, not tested). `written_form_of_every_record_kind` creates its account without them, so `tests/sync_format/v4.json` pins both as `null`: a change of their type would not trip the format test of the next version. SYN-023: nothing checks that recording a fetch failure or a fetch-log mark leaves the change log unchanged. SYN-030: nothing checks a device writes only in its own area.
- User value: Nothing visible — a renamed account on an older device can never silently drop the wrapper set on a newer one.
- Done when: a two-device test with a format-3 rename over an account with a wrapper, both kept; the generator fills `country` and `wrapper` (the next snapshot pins them as strings; v4 untouched); a test that device-local writes record no change; a test that publishing touches only the device's area.
