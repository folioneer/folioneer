# TODO-102 — (release) — Signed installers on Windows and macOS

From gh283 (owner, 2026-10-17), which this entry closes when it ships. The Windows installer is unsigned, so SmartScreen warns every new user, and there is no macOS build. The Linux AppImage and deb exist; Flathub only if Linux users ask (owner, 2026-10-17).

**User value:** A new user installs Folioneer on Windows or macOS without a security warning and receives updates without reinstalling.
**Done when:** The Windows installer is signed (SignPath Foundation first, a paid OV certificate if refused) and installs without the unknown-publisher warning; a universal macOS build, signed and notarised, is attached to each release and updates itself; the release workflow builds and signs all of them, and no signing material appears in a log.
**Design:** none
**Open questions:**

- [x] Windows signing? — SignPath Foundation first; a paid OV certificate if refused. (owner, 2026-10-17)
- [x] Flathub at launch? — Later, if Linux users ask. (owner, 2026-10-17)
