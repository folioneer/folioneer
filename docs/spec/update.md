# Business Rules — Application Update

## Context

The application is distributed as a desktop executable. When a new version is published, the user must be informed so they can install it without manual checking. This feature covers detecting an available update, proposing it to the user, downloading, installing, and guarantees on user data.

---

## Business rules

### Discovery

**UPD-001 (was R1) — Automatic check at startup (backend)**: At each launch, a background check runs to detect whether a new version is available. It starts once the UI is fully loaded.

**UPD-002 (was R2) — No notification when no update (frontend)**: If the UPD-001 check detects no new version, nothing is shown to the user and the application stays in its normal state.

### Notification

**UPD-003 (was R3) — Update banner content (frontend)**: If a new version is detected, a fixed banner is displayed in the application shell, indicating the available version number and offering two actions: "Install" and "Dismiss".

**UPD-004 (was R4) — Banner persistence (frontend)**: The banner is integrated into the permanent shell layout and remains visible across all application pages without interrupting navigation.

**UPD-005 (was R5) — User-initiated banner dismissal (frontend)**: Clicking "Dismiss" or the × button closes the banner. This triggers the postponement behavior described in UPD-019.

### Download

**UPD-006 (was R6) — Download trigger (frontend + backend)**: When the user clicks "Install", the update download starts in the background.

**UPD-007 (was R7) — Non-blocking during download (frontend)**: During the download, the application remains fully navigable.

**UPD-008 (was R8) — Progress in the banner (frontend)**: During the download, the banner displays progress. The "Install" button is replaced by the progress indicator and cannot be re-triggered.

**UPD-009 (was R9) — Download integrity (backend)**: Before allowing installation, the application verifies the integrity of the downloaded file via a checksum. If verification fails, the file is considered corrupted and the UPD-023 error flow applies.

**UPD-010 (was R10) — Concurrent download (backend)**: If a newer version is published while a download is already in progress, the in-progress download continues without interruption. The newer version will be offered at the next launch per UPD-020.

### Ready to install

**UPD-011 (was R11) — "Ready to install" banner content (frontend)**: When the download is complete and integrity is verified, the banner displays "Ready to install" and a "Restart now" button.

**UPD-012 (was R12) — Banner non-dismissible after download (frontend)**: In the "Ready to install" state, the banner no longer offers a × button or a "Dismiss" option. It remains visible until restart or application close.

### Installation

**UPD-013 (was R13) — Restart and installation (frontend + backend)**: When the user clicks "Restart now", the update is installed and the application restarts automatically.

**UPD-014 (was R14) — User data preservation (backend)**: The update only replaces the application executable. All user data (assets, accounts, categories, prices, operations) is preserved intact after restart.

**UPD-015 (was R15) — Backward compatibility (backend)**: Each new version guarantees compatibility with data produced by any earlier version. No update may introduce a change incompatible with an existing schema or data. UPD-016 migrations may only extend or adapt the schema, never destructively remove or modify data.

**UPD-016 (was R16) — Automatic schema migration (backend)**: If the new version introduces database schema changes, migrations are applied automatically at the first launch after the update, before the UI is accessible. If multiple versions have been skipped, all intermediate migrations are applied in order, without exception.

**UPD-017 (was R17) — Loading screen during migrations (frontend)**: During the UPD-016 migration phase, a loading screen is displayed with a message indicating that the database update is in progress.

**UPD-018 (was R18) — Migration failure (backend)**: If a migration fails at startup, the application displays a critical error message and refuses to start in order to protect data integrity. The user is invited to contact support.

### Postponement

**UPD-019 (was R19) — Postponement to next launch (frontend)**: If the user closes the UPD-003 banner ("Dismiss" or ×), or closes the application from the UPD-011 state without restarting, the update is offered again at the next launch.

**UPD-020 (was R20) — Priority to the most recent version (backend)**: If an even more recent version is available at the next launch, that version is offered, not the previously dismissed one.

### Errors

**UPD-021 (was R21) — Silent check failure (frontend)**: If the startup check fails (no network, server unavailable), no notification is displayed and the application starts normally.

**UPD-022 (was R22) — Logging check failures (backend)**: Any error during the startup check is logged in the application logs.

**UPD-023 (was R23) — Download error display (frontend)**: If the download fails (network error, insufficient disk space, or any other cause), or if the UPD-009 checksum fails, the banner displays an error message and a "Retry" button.

**UPD-024 (was R24) — Retry action (frontend + backend)**: Clicking "Retry" restarts the download from the beginning.

### Manual check

**UPD-025 (was R25) — Manual check entry point (frontend)**: The "About" page exposes the current version number and a "Check for updates" button. When the user clicks this button, a check is triggered using the same mechanism as UPD-001.

**UPD-026 (was R26) — Manual check loading state (frontend)**: During the check triggered by UPD-025, the "Check for updates" button is disabled and shows a loading indicator to prevent multiple triggers.

**UPD-027 (was R27) — Manual check result (frontend)**: At the end of the UPD-025 check: if an update is available, the UPD-003 banner is displayed; if no update is available, a message on the "About" page confirms that the application is up to date.

---

## Workflow

```
[App startup → UI loaded]                          [About page → "Check" button]
  → Background check (UPD-001)                       → Check + spinner (UPD-025, UPD-026)
        │                                                   │ (same flow as UPD-001)
        ├─ No update → nothing displayed (UPD-002) ────────┤
        │                                             └─ Up to date → confirmation message (UPD-027)
        ├─ Network/server error → silent log, nothing displayed (UPD-021, UPD-022)
        │
        └─ New version available
              → Banner: "Version X.Y.Z available" + [Install] [Dismiss] (UPD-003, UPD-004)
                    │
                    ├─ [Dismiss / ×] → banner disappears, re-offered at next launch (UPD-005, UPD-019)
                    │
                    └─ [Install] → background download, app navigable (UPD-006, UPD-007)
                          → Progress visible in the banner (UPD-008)
                          → If newer version published → keep going (UPD-010)
                                │
                                ├─ Failure → error banner + [Retry] (UPD-023)
                                │            [Retry] → restarts from the beginning (UPD-024)
                                │
                                └─ Success → checksum verification (UPD-009)
                                        │
                                        ├─ Checksum KO → error banner + [Retry] (UPD-023, UPD-024)
                                        │
                                        └─ Checksum OK → "Ready to install" + [Restart] (UPD-011)
                                                         persistent banner, non-dismissible (UPD-012)
                                                │
                                                ├─ [App closed] → re-offered at next launch (UPD-019)
                                                │
                                                └─ [Restart now] (UPD-013)
                                                      → Data preserved (UPD-014)
                                                      → DB migration if needed (UPD-016, UPD-017)
                                                           ├─ Failure → critical error, app blocked (UPD-018)
                                                           └─ Success → installation + restart (UPD-013)
```

---

## UX Mockup

### Entry points

Two entry points:

1. **Automatic** — triggered at startup, once the UI is fully loaded (UPD-001).
2. **Manual** — "Check for updates" button on the "About" page (UPD-025).

### Main component

Fixed banner integrated into the application shell (header or footer), visible across all pages. It only appears while an update is in progress or available, and disappears once the application has restarted. It is not a dialog or an ephemeral notification: the banner is part of the permanent layout.

### Banner states

- **Absent**: no update detected — banner is not rendered (UPD-002)
- **Update available**: "Version X.Y.Z available" + "Install" and "Dismiss" buttons (UPD-003)
- **Download in progress**: progress indicator, "Install" button replaced (UPD-008)
- **Ready to install**: "Ready to install" + "Restart now" button — non-dismissible (UPD-011, UPD-012)
- **Download error**: error message + "Retry" button (UPD-023)
- **Migration in progress**: loading screen outside the banner ("Updating database…") (UPD-017)
- **Migration error**: critical error message, application blocked at startup, outside the banner (UPD-018)
- **Up to date**: contextual message on the "About" page, outside the banner (UPD-027)

### User flow

1. The application launches, the UI is displayed, then a background check runs.
2. If a newer version is available, a banner appears with the version number.
3. The user clicks "Install" → background download, progress visible, navigation free.
4. Download complete → persistent "Ready to install" banner + "Restart now" button.
5. The user clicks "Restart now" when ready → migrations if any → installation → restart.

---

## Future features

- **Critical updates**: some versions could be marked as critical (security flaw, data corruption), forcing a mandatory update before the application can be used. Not included in this version.
- **Release notes**: at the first launch after an update, display the release notes (changelog) of the newly installed version. Covered by the What's New dialog — see [whats-new.md](whats-new.md) (WNW).

---

## Open questions

None — all questions have been resolved.
