# 2026-10-03 — DEBT-066 — An asset's name, reference and category have no length or character rule

- Found by: reviewer-security on TODO-055
- Where: `Asset::validate` in `src-tauri/src/context/asset/domain/asset.rs`; `AssetService::add_named_asset` (the command line's path, with the same rules)
- Severity: 🔵
- Observation: the asset rules check that a name and a reference are not empty, and nothing else: no maximum length, and a control character is accepted from the window's form or from another device's synced change. The command line escapes what it prints.
- User value: None directly — a name pasted with a stray control character or of unreasonable length is refused where it is typed.
- Done when: `Asset::validate` refuses a control character and a name or reference above a stated length, with a typed error the form shows and the command line reports.
