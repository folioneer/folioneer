# Plan — #033 Carry the owner's data over from the application's former name

Entry: `docs/todo.md` #033.

## Facts the plan rests on (measured 2026-09-20)

- The data folder follows the **identifier**: Linux `~/.local/share/<id>/`, Windows `%LOCALAPPDATA%\<id>\`. It holds the database (`portfolio`, `portfolio-wal`, `portfolio-shm`), `logs/`, and the webview's storage — on Linux `localstorage/` and `storage/` (theme, language, auto-fetch and auto-record switches, drawer and folded-section states) plus caches; on Windows `EBWebView/`. The window position lives in the config folder (`~/.config/<id>/.window-state.json`, `%APPDATA%\<id>\`).
- Tauri's Windows installer keys an installation on the **product name**: the Folioneer installer installs beside the old program. The old program's uninstaller deletes application data only when its checkbox is ticked.
- The old OS scheduler entries carry the old name: systemd unit `vaultcompass-fetch`, Windows task `VaultCompassFetch`. Start-up self-repair (SPF-015) registers the schedule under the current names.
- The sync folder's header file is `vaultcompass-sync.json`, a format constant: every device sharing a portfolio reads it.

## Carry-over, per computer

After the first Folioneer release, application closed: run the script (`scripts/migration/from-vaultcompass.sh` or `.ps1`), uninstall the old program (Windows: "delete application data" left unticked; Linux: remove the old AppImage), install Folioneer, open it, check the portfolio, the sync and the scheduled download. On the Linux computer the agent runs the copy itself, on the owner's go-ahead, reading the old folder and never writing to it. The PowerShell script has been reviewed but never executed: its first run is the owner's, on Windows, and it refuses to overwrite anything.

## Closure

Once the owner confirms both computers: the two scripts, their tests, the README section "Coming from VaultCompass", this plan and the entry are removed.
