# TODO-081 — (release) — Claude Desktop connects to Folioneer from one file, without a block of JSON

Raised by the owner on 2026-10-11. To connect Claude Desktop today the owner edits `claude_desktop_config.json` by hand and pastes a block naming the program's path (README, § Agents). No agent client finds a local application that is running: a client knows only the local servers registered in its own configuration. What can be made simple is that registration.

Claude Desktop installs a local server from a bundle (`.mcpb`, formerly `.dxt`): a file holding a manifest, opened by a double-click and installed by one confirmation. The format takes a native program. Folioneer's bundle would hold a manifest only, pointing at the installed program started with `--mcp` — the bridge of ADR-023, unchanged: no network port, nothing listening.

To read when this is scheduled, none of it verified: the manifest's fields for a program that is installed apart from the bundle, and whether its path is fixed, found, or asked of the user at install; what differs between the Windows installer, the AppImage and the `.deb`; whether a bundle has to be signed.

**Size:** M — one pull request: the manifest, the release workflow that attaches the bundle, the README.
**User value:** The owner connects Claude Desktop to Folioneer by opening a file and confirming, without editing a configuration file.
**Done when:** each release carries a `.mcpb` bundle beside its installers; opened in Claude Desktop on a computer where that release is installed, it installs without editing a file, and Claude Desktop then lists Folioneer's tools; the README says so, the block of JSON kept for the other clients; a test holds that the manifest names the tools' program as the release installs it.
**Design:** none
**Open questions:** none
