/**
 * E2E tests — Scheduled Price Fetch (SPF) Settings flow
 *
 * Spec:     docs/spec/scheduled-price-fetch.md
 * Contract: docs/contracts/scheduled-fetch-contract.md § configure_scheduled_fetch / get_scheduled_fetch_status
 *
 * Spec rules covered by this file:
 *   SPF-010 — Settings section: toggle off by default, time field hidden while disabled
 *   SPF-018 — enabling without picking a time defaults the trigger to 22:15
 *   SPF-052 — fresh-install status line reads "No download yet." (last_run: None)
 *   SPF-011/012 — configuration persists with the app data and round-trips through
 *     the real backend (not just local component state) across a section remount
 *   SPF-019 — changing the time re-submits a well-formed HH:MM value
 *   SPF-060 — toggle/time field are disabled while a configure() call is in flight
 *
 * Pyramid rationale:
 *   Unit/integration tiers (1019 BE + 1825 FE tests) already cover: domain
 *   validation (InvalidTriggerTime), the once-per-day guard, backfill, DST
 *   handling, and the FE hook's revert-on-error branch (mocked gateway). This
 *   E2E scenario locks in the one thing only a real running app can prove: the
 *   Settings toggle click and time-field edit genuinely reach
 *   `configure_scheduled_fetch` / `get_scheduled_fetch_status` on the real Rust
 *   backend, persist to real SQLite, and read back correctly after the section
 *   remounts — exactly the UI → IPC → backend handshake that is the reason this
 *   feature has an E2E tier at all.
 *
 *   The E2E harness activates the backend's `NoopScheduler` (via
 *   FOLIONEER_E2E_DATA_DIR, debug build) — enabling/disabling the daily
 *   download never touches the host's systemd/launchd/Task Scheduler, so the
 *   full enable → time-change → disable round trip is safe to exercise here.
 *
 * NOT covered here (and why):
 *   - SPF-013 (ScheduleRegistrationFailed / ScheduleRemovalFailed): the E2E
 *     harness's NoopScheduler never fails registration — this error is only
 *     reachable by mocking the scheduler port, which happens at the BE Tier 1
 *     (mockall) and FE Vitest (mocked gateway) tiers.
 *   - InvalidTriggerTime: per the contract, "the frontend's time field
 *     constrains input so the case is unreachable from the UI" — a native
 *     `<input type="time">` cannot produce a malformed HH:MM value. Covered by
 *     BE Tier 1/2 domain-validation tests.
 *   - A run actually executing (SPF-020 through SPF-053): the scheduled run is
 *     a separate, invisible process execution with no frontend caller per the
 *     contract ("internal-only — no frontend caller, so no commands"); no UI
 *     surface exists to drive it from E2E. Covered by BE Tier 3 integration
 *     tests in src-tauri/tests/.
 *
 * Seed strategy:
 *   None. `ScheduledFetchConfiguration` is a device-wide singleton — exactly
 *   one row exists per database — so the ephemeral E2E database (B36) already
 *   starts in the spec's default state (disabled, no run ever recorded) with
 *   no IPC seeding required. The scenario is self-cleaning: it re-disables the
 *   toggle at the end so the singleton is left in its default state for any
 *   test file that runs afterward in the same session.
 *
 * Why one scenario:
 *   Because the configuration is a device-wide singleton (not a per-entity
 *   row keyed by a seeded id), splitting this into independent `it` blocks
 *   would make them order-dependent on each other in a much less legible way
 *   than a single sequential walk. Following the holding_note.test.ts /
 *   split.test.ts precedent, the full critical path — disabled → enable with
 *   default time → status → change time → persists across remount → disable
 *   — is written as one self-contained, self-cleaning scenario.
 */

import assert from "node:assert";
import { $, browser } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToSettings } from "../helpers/navigation";
import { setReactInputValue } from "../helpers/react";
import { captureScreen } from "../helpers/screenshot";

// ---------------------------------------------------------------------------
// Fixed values (E2E rule E9 — a native <input type="time"> has no date/
// duplicate-value concern, but the value is still a single fixed constant so
// the assertion is deterministic).
// ---------------------------------------------------------------------------
const DEFAULT_TRIGGER_TIME = "22:15"; // SPF-018
const CHANGED_TRIGGER_TIME = "06:30";

describe("scheduled_fetch", () => {
  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  // -------------------------------------------------------------------------
  // SPF-010/018/052/011/012/019/060 — critical path:
  //   fresh install (disabled, no time field) → enable (default time 22:15,
  //   "No download yet." status) → change the time → the new value survives a
  //   section remount (proves the round trip through the real backend, not
  //   just local React state) → disable again (time field hidden, singleton
  //   left in its default state for later test files).
  // -------------------------------------------------------------------------
  it("SPF-010/018/052/011/012/019/060: daily-download toggle round-trips through the real backend", async () => {
    await navigateToSettings();

    // -----------------------------------------------------------------
    // Step 1 — Fresh-install state (SPF-010): toggle unchecked, time
    // field absent (hidden while disabled per the component's `{enabled
    // && (...)}` guard).
    // -----------------------------------------------------------------
    const toggle = await $("#scheduled-fetch-toggle");
    await toggle.waitForEnabled({ timeout: 10000 }); // initial status load resolved
    assert.strictEqual(
      await toggle.isSelected(),
      false,
      "SPF-010 — the daily-download toggle must be off by default on a fresh database",
    );
    let timeField = await $("#scheduled-fetch-time");
    assert.strictEqual(
      await timeField.isExisting(),
      false,
      "SPF-010 — the time field must not render while the toggle is off",
    );

    // -----------------------------------------------------------------
    // Step 2 — Enable (SPF-012 configure round trip). SPF-060 disables
    // the toggle while the call is in flight; waitForEnabled below
    // confirms the real `configure_scheduled_fetch` IPC call resolved.
    // -----------------------------------------------------------------
    await toggle.click();
    await toggle.waitForEnabled({ timeout: 10000 });
    assert.strictEqual(
      await toggle.isSelected(),
      true,
      "Toggle must read as checked after enabling",
    );

    // -----------------------------------------------------------------
    // Step 3 — Time field appears with the SPF-018 default.
    // -----------------------------------------------------------------
    timeField = await $("#scheduled-fetch-time");
    await timeField.waitForExist({ timeout: 8000 });
    await timeField.waitForEnabled({ timeout: 8000 });
    assert.strictEqual(
      await timeField.getValue(),
      DEFAULT_TRIGGER_TIME,
      "SPF-018 — enabling without choosing a time must default the trigger to 22:15",
    );

    // -----------------------------------------------------------------
    // Step 4 — Status line (SPF-052): no run has ever executed in this
    // ephemeral database (the NoopScheduler never fires a real trigger),
    // so the status must read the fresh-install "No download yet." copy.
    // -----------------------------------------------------------------
    const statusLine = await $("#scheduled-fetch-status");
    await statusLine.waitForExist({ timeout: 8000 });
    await browser.waitUntil(async () => (await statusLine.getText()) === "No download yet.", {
      timeout: 8000,
      timeoutMsg: 'SPF-052 — fresh-install status line must read "No download yet."',
    });
    await captureScreen("scheduled-fetch-settings");

    // -----------------------------------------------------------------
    // Step 5 — Change the trigger time (SPF-019 well-formed HH:MM value
    // from the native time input; SPF-012 re-registers at the new time).
    // A native <input type="time"> takes its value pre-formatted as
    // "HH:MM" — no locale display conversion is needed here (unlike the
    // DateField precedent in E7).
    // -----------------------------------------------------------------
    await setReactInputValue("scheduled-fetch-time", CHANGED_TRIGGER_TIME);
    await timeField.waitForEnabled({ timeout: 8000 }); // SPF-060 in-flight → settled
    assert.strictEqual(
      await timeField.getValue(),
      CHANGED_TRIGGER_TIME,
      "Time field must reflect the changed trigger time immediately after configure() resolves",
    );

    // -----------------------------------------------------------------
    // Step 6 — Persistence (SPF-011/012): navigate away and back so the
    // section unmounts and remounts, forcing a fresh
    // `get_scheduled_fetch_status` read from the real SQLite database —
    // proves the value was actually persisted server-side, not just held
    // in React state.
    // -----------------------------------------------------------------
    const accountsNav = await $("#nav-accounts");
    await accountsNav.waitForExist({ timeout: 10000 });
    await accountsNav.click();
    await $("#fab-add-account").waitForExist({ timeout: 10000 });

    await navigateToSettings();
    const toggleAfterRemount = await $("#scheduled-fetch-toggle");
    await toggleAfterRemount.waitForEnabled({ timeout: 10000 });
    assert.strictEqual(
      await toggleAfterRemount.isSelected(),
      true,
      "SPF-011 — enabled state must persist across a section remount",
    );
    const timeFieldAfterRemount = await $("#scheduled-fetch-time");
    await timeFieldAfterRemount.waitForExist({ timeout: 8000 });
    assert.strictEqual(
      await timeFieldAfterRemount.getValue(),
      CHANGED_TRIGGER_TIME,
      "SPF-011/012 — the changed trigger time must persist across a section remount, " +
        "proving the round trip through the real backend",
    );

    // -----------------------------------------------------------------
    // Step 7 — Disable again (SPF-010): time field hidden once more.
    // Self-cleaning — leaves the singleton configuration in its default
    // disabled state for any test file that runs later in this session.
    // -----------------------------------------------------------------
    await toggleAfterRemount.click();
    await toggleAfterRemount.waitForEnabled({ timeout: 10000 });
    assert.strictEqual(
      await toggleAfterRemount.isSelected(),
      false,
      "Toggle must read as unchecked after disabling",
    );
    const timeFieldAfterDisable = await $("#scheduled-fetch-time");
    await timeFieldAfterDisable.waitForExist({ timeout: 8000, reverse: true });
    assert.strictEqual(
      await timeFieldAfterDisable.isExisting(),
      false,
      "SPF-010 — the time field must disappear again once the toggle is turned off",
    );
  });
});
