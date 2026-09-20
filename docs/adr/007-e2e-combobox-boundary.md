# ADR-007: E2E Test Boundary at HeadlessUI ComboboxField

## Status

Accepted

## Context

The app uses [HeadlessUI v2](https://headlessui.com/) `Combobox` components for searchable dropdowns (asset selection in buy, open-balance, and other forms). E2E tests run in WebKitGTK via tauri-driver + WebdriverIO.

Two problems make ComboboxField non-automatable in this environment:

1. **`isTrusted = false`**: HeadlessUI v2 guards state transitions on `event.isTrusted`. Programmatic events dispatched by WebdriverIO (via `setReactInputValue` / `setValue`) are synthetic and carry `isTrusted = false`, so the combobox never opens its dropdown.

2. **floating-ui portal clipping**: The dropdown is rendered in a portal anchored to the document body. In the WebKitGTK WebView used by tauri-driver, the portal's absolute position often falls outside the area that WebdriverIO considers interactable, making option elements unclickable even when visible in screenshots.

Both issues were first documented in an analogous Tauri 2 + React 19 + WebKitGTK codebase. Investigation confirmed the same failure mode applies here, so the same boundary policy is adopted.

## Decision

**E2E tests stop at the ComboboxField boundary.**

- Tests that would require typing into a ComboboxField to proceed are redesigned to avoid the interaction:
  - If the component can be opened with the asset **pre-populated** (e.g. BuyTransactionModal opened from a holding row), seed data via IPC first so the pre-populated path is used.
  - If the form cannot be submitted without ComboboxField interaction (e.g. OpenBalanceModal), the submit-path test is replaced with a direct IPC invocation (`browser.executeAsync` → `window.__TAURI_INTERNALS__.invoke(...)`) that exercises the backend command end-to-end.
  - Frontend guard checks (e.g. "submit disabled when date is in the future") are retained; the ADR limitation is noted in a comment. The partial assertion (submit is disabled) still provides value even when the form is also incomplete because of the missing asset.

- The full form-submit UI flow (all fields filled → submit → modal closes) is covered by RTL component tests (`vitest` + `@testing-library/react`) which run in jsdom and are not affected by the `isTrusted` constraint.

## Consequences

- E2E suites that touch ComboboxField-gated flows are smaller and more stable.
- The backend contract for those commands is still exercised end-to-end via IPC tests in the same E2E suite.
- Any future replacement of HeadlessUI Combobox with a native `<select>` or a WebKit-compatible alternative would allow the IPC tests to be promoted back to full UI tests without architectural changes.
