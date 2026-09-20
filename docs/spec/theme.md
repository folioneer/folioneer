# Business Rules — Interface Theme (theme)

## Context

The interface offers three display modes: light, dark and automatic. The mode is controlled by a button in the header, stored locally, and restored at every start.

---

## Business rules

**R1 — Available modes**: Three modes are available: `day` (always light), `night` (always dark), `auto` (follows the operating system's preference).

**R2 — Toggle cycle**: The header button rotates the modes in the order `day → night → auto → day`. The icon reflects the current mode: sun (`day`), moon (`night`), monitor (`auto`).

**R3 — Persistence**: The selected mode is stored in `localStorage` under the key `theme-mode` and restored when the application starts. With no stored value, the mode is `auto`.

**R4 — Auto mode**: In `auto` mode the theme follows `prefers-color-scheme: dark`. The interface reacts in real time to a change of the system preference (for example macOS switching at sunset), without a reload.

**R5 — Applying the theme**: The light theme is the default state (the base `@theme` tokens in `tailwind.css`). The `.dark` class is set on `<html>` only in `night` mode, or in `auto` mode when the operating system is dark. In `day` mode the `.dark` class is removed from `<html>`.

**R6 — Header adapted to the dark theme**: The header uses gradient tokens (`--color-header-from` / `--color-header-to`) that adapt to the dark theme with a deeper indigo (`#21005D → #381E72` in the dark theme, `#4F378A → #6750A4` in the light one). The brand identity holds in both, and the white text stays accessible (contrast above 7:1, WCAG AA).

> **Waiver — no automated test for R6**: Asserting static CSS hex values in an automated test would be trivial and brittle (it depends on the build tooling). R6 is verified visually in the light and dark themes. No test is required for this rule.

---

## Workflow

```
[The user clicks the theme button]
  → Next mode in the cycle (day → night → auto → day)
  → Stored in localStorage
          │
          ▼
[.dark class added to / removed from <html>]
  → Every M3 token switches through tailwind.css
  → The header switches to a deeper indigo (dark tokens)
```

```
[The application starts]
  → Reads localStorage["theme-mode"]
  → Fallback: auto
          │
          ▼ (when auto)
[Reads prefers-color-scheme]
  → Listens to operating-system changes in real time
```
