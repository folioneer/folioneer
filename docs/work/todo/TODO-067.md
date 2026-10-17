# TODO-067 — (frontend) — The application says how to connect an agent once the setting is on

Raised by the owner on 2026-10-11, at the 0.8.0 release: he switches "Allow agents to connect" on and the application tells him nothing more. How to plug an agent in — the `claude mcp add` command, the entry of Claude Desktop's configuration file with the installed program's full path — is written only in `README.md`, and for Windows without a block ready to paste. The application has no help page of any kind.

**User value:** The owner connects Claude Code or Claude Desktop from what the application shows him, without leaving it to look for a document.
**Done when:** with the setting on, the application shows what to give each agent client, with the path of the program as installed on this computer and a way to copy it; the same text is right on Linux (package and AppImage) and on Windows; `README.md` has the Windows block for Claude Desktop.
**Design:** none
**Open questions:**

- [x] Where does it show: beside the checkbox on the settings page, or in a help menu of the application, which does not exist yet and would hold more than this? — Beside the checkbox on the settings page; the first run (gh284) points to it. (owner, 2026-10-17)
- [x] Which clients are named: Claude Code and Claude Desktop only, or a general form for any client as well? — Claude Desktop and Claude Code ready to copy, plus a general form for any MCP client. (owner, 2026-10-17)
