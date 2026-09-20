/**
 * E2E tests — Multi-Device Sync (SYN) Settings flow, single-device critical path
 *
 * Spec:     docs/spec/multi-device-sync.md
 * Contract: docs/contracts/sync-contract.md § enable_sync / sync_now / pause_sync /
 *           resume_sync / rename_sync_device / leave_sync / get_sync_status
 *
 * Spec rules covered by this file:
 *   SYN-010 — opt-in: disabled state offers "Enable sync"; enabling shows the status block
 *   SYN-011 — enabling requires a folder and a passphrase; two-step modal
 *   SYN-012 — passphrase minimum length (12) gates the submit button; advisory strength
 *     indicator renders once the minimum is met
 *   SYN-018 — device name is required; gates the submit button independently of the
 *     passphrase gate
 *   SYN-061 — "Sync now" runs a sync immediately
 *   SYN-063 — sync status assembly: device name, folder, roster/failure/held-back
 *     surfaces render from the real `SyncStatus` the backend returns
 *   SYN-070 — pause on this device
 *   SYN-072 — rename device; the manifest is republished (asserted indirectly — the new
 *     name reads back from `get_sync_status`, not local state)
 *   SYN-073 — resume: a paused device resumes without re-entering the passphrase
 *   SYN-082 — leave sync: the section returns to the disabled state; the device's folder
 *     area (its segment file) is not deleted, only its manifest
 *
 * Pyramid rationale:
 *   Unit/integration tiers already cover: `SyncFolderState`/`FolderProblem` detection,
 *   the join/rebuild branch (SYN-014/015/036/080), encryption and key derivation
 *   (SYN-050-055), held-back changes and conflict resolution (CFR-010..060), and every
 *   precondition-guard error variant (`AlreadyEnabled`, `SyncPaused`, `NotPaused`, …) —
 *   see `PortfolioSyncOrchestrator`'s and `SyncService`'s inline `#[cfg(test)]` modules
 *   and the Tier-3 two-device integration test. This E2E scenario locks in the one thing
 *   only a real running app can prove: the Settings section's enable → status → sync now
 *   → pause → resume → rename → leave chain genuinely reaches `enable_sync` /
 *   `sync_now` / `pause_sync` / `resume_sync` / `rename_sync_device` / `leave_sync` /
 *   `get_sync_status` on the real Rust backend, and that the backend actually writes the
 *   encrypted shared folder on disk (the header file and a device area) — not just an
 *   in-memory mock.
 *
 * NOT covered here (and why — see plan § Halt Artifact H1):
 *   - The join path (SYN-014/036, `InstallationHoldsUserData`, `HistoryIncomplete`,
 *     `RebuildInterrupted`) — H1: `wdio.conf.ts` launches exactly one app instance with
 *     one `FOLIONEER_E2E_DATA_DIR`; there is no second-device/second-profile E2E
 *     helper. Covered by `src-tauri/tests/sync_two_devices.rs` (Tier 3).
 *   - "Start over" (SYN-071/053) and folder-picker (`#sync-enable-browse`) — H2: the
 *     Browse button opens a native GTK dialog WebDriver cannot drive; the folder is
 *     always typed directly in this file. Start-over reuses the same enable-modal code
 *     path already exercised here and is otherwise a destructive, rarely-taken branch —
 *     left to its own component tests (`EnableSyncModal.test.tsx`).
 *   - `change_sync_folder` (SYN-074) and conflict notices (SYN-066) — no scenario in the
 *     single-device chain naturally produces a second folder or a conflict; both are
 *     fully covered by BE Tier 1/2 and the FE `useSyncSection.test.ts` /
 *     `NoticeList.test.tsx` mocked-gateway tests.
 *   - SYN-017 (honest-positioning copy) and the no-recovery/metadata-exposure notes
 *     (SYN-053/054) — the paragraphs carrying this copy in `SyncSection.tsx` and
 *     `EnableSyncModal.tsx` have no stable `id` (they are plain `<p>{t(...)}</p>` nodes).
 *     E1-E4 forbid a text/aria-label selector, so this rule has no locale-invariant E2E
 *     surface; it is exercised by `SyncSection.test.tsx` / `EnableSyncModal.test.tsx`,
 *     which can assert translated text directly. Not a missing-helper gap — adding an
 *     `id` to those paragraphs is a frontend source change, out of this writer's scope.
 *   - `SyncFailure` rendering (SYN-034/035/069/084), inconsistent holdings (SYN-040),
 *     and held-back changes (SYN-041) — none is reachable from a single, freshly
 *     enabled device with an always-available folder and no other device publishing
 *     concurrently; these require the join/two-device setup this file cannot drive.
 *   - The shell `SyncIndicator` (`#sync-indicator`) — it reads the same `SyncStatus`
 *     already exercised through `SyncSection`; asserting it too would duplicate
 *     coverage without proving anything new about the IPC round trip.
 *
 * Seed strategy:
 *   None via IPC. `SyncDevice` is a device-wide singleton, like `ScheduledFetchConfiguration`
 *   (see `scheduled_fetch.test.ts`) — the ephemeral E2E database starts disabled, and no
 *   other spec file in this suite touches sync commands. The one thing this file does
 *   seed is the shared folder itself: a real empty directory created with Node's `fs`
 *   (not through Tauri IPC), because `enable_sync`'s first-device branch needs a real,
 *   writable path on disk to publish into — there is no seed command that could stand
 *   in for it.
 *
 * Why one scenario:
 *   Every step after "enable" depends on the device row the previous step created —
 *   splitting into independent `it` blocks would only reintroduce that ordering
 *   dependency through shared `before` state instead of removing it. Following the
 *   `scheduled_fetch.test.ts` precedent, the full critical path is written as one
 *   self-contained, self-cleaning scenario: it leaves sync disabled again at the end
 *   (SYN-082) so the singleton is back in its default state for any test file that runs
 *   afterward in the same session.
 */

import assert from "node:assert";
import { existsSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { $, browser } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToSettings } from "../helpers/navigation";
import { setReactInputValue } from "../helpers/react";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount } from "../helpers/seed";

// ---------------------------------------------------------------------------
// Fixed values (E2E rule E9 — no dates are involved here, but every value that
// drives a real write is still a single fixed constant for deterministic
// assertions).
// ---------------------------------------------------------------------------
const TOO_SHORT_PASSPHRASE = "shortpass1"; // 10 chars — below the SYN-012 minimum of 12
const PASSPHRASE = "correct horse battery staple 2020"; // well above the 12-char minimum
const FIRST_DEVICE_NAME = "Desktop";
const RENAMED_DEVICE_NAME = "Laptop";

describe("sync", () => {
  let syncFolder: string;

  before(async () => {
    // A real, empty, writable directory — enable_sync's first-device branch (SYN-013)
    // needs a genuine path on disk; there is no seed command that stands in for it.
    syncFolder = mkdtempSync(join(tmpdir(), "folioneer-sync-"));
    // SYN-013 — the first segment carries one Created change per existing synced record;
    // one account is enough for a segment to exist at all.
    await seedAccount("Sync E2E account");
  });

  after(() => {
    if (existsSync(syncFolder)) {
      rmSync(syncFolder, { recursive: true, force: true });
    }
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  // -------------------------------------------------------------------------
  // SYN-010/011/012/018/061/063/070/072/073/082 — critical path:
  //   fresh install (disabled) → open the enable modal → folder step (real
  //   inspect_sync_folder round trip) → passphrase step (SYN-012/018 gates) →
  //   submit reaches the real enable_sync and writes the encrypted folder on
  //   disk → status block renders → "Sync now" → pause → resume → rename →
  //   leave (folder area survives, singleton left disabled for later files).
  // -------------------------------------------------------------------------
  it("SYN-010/011/012/018/061/063/070/072/073/082: enable as the first device round-trips through the real backend", async () => {
    await navigateToSettings();

    // -----------------------------------------------------------------
    // Step 1 — Fresh-install state (SYN-010): "Enable sync" offered, no
    // status block yet.
    // -----------------------------------------------------------------
    const enableBtn = await $("#sync-enable");
    await enableBtn.waitForExist({ timeout: 10000 });
    assert.strictEqual(
      await (await $("#sync-status")).isExisting(),
      false,
      "SYN-010 — the status block must not render while sync is disabled",
    );

    // -----------------------------------------------------------------
    // Step 2 — Open the enable modal (SYN-011), step 1 of 2: the folder.
    // -----------------------------------------------------------------
    await enableBtn.click();
    const modal = await $("#sync-enable-modal");
    await modal.waitForExist({ timeout: 8000 });
    const form = await $("form#sync-enable-form");
    await form.waitForExist({ timeout: 8000 });

    const folderField = await $("#sync-enable-folder");
    await folderField.waitForExist({ timeout: 8000 });
    const nextBtn = await $("#sync-enable-next");
    await nextBtn.waitForExist({ timeout: 8000 });
    assert.strictEqual(
      await nextBtn.isEnabled(),
      false,
      "SYN-011 — Next must stay disabled before a folder has been inspected",
    );

    // Typing the folder fires the real inspect_sync_folder IPC call (never
    // click #sync-enable-browse — H2: it opens a native dialog WebDriver
    // cannot drive).
    await setReactInputValue("sync-enable-folder", syncFolder);
    await nextBtn.waitForEnabled({
      timeout: 10000,
    }); // real inspect_sync_folder round trip resolved: the empty folder is usable

    await nextBtn.click();

    // -----------------------------------------------------------------
    // Step 3 — Passphrase step (SYN-011/012/018): both twice-entered
    // passphrase fields for a first device, plus the required device name.
    // -----------------------------------------------------------------
    const passphraseField = await $("#sync-enable-passphrase");
    await passphraseField.waitForExist({ timeout: 8000 });
    const passphraseConfirmField = await $("#sync-enable-passphrase-confirm");
    await passphraseConfirmField.waitForExist({ timeout: 8000 });
    const deviceNameField = await $("#sync-enable-device-name");
    await deviceNameField.waitForExist({ timeout: 8000 });
    const submitBtn = await $("#sync-enable-submit");
    await submitBtn.waitForExist({ timeout: 8000 });

    await setReactInputValue("sync-enable-device-name", FIRST_DEVICE_NAME);
    await setReactInputValue("sync-enable-passphrase", TOO_SHORT_PASSPHRASE);
    await setReactInputValue("sync-enable-passphrase-confirm", TOO_SHORT_PASSPHRASE);
    assert.strictEqual(
      await submitBtn.isEnabled(),
      false,
      "SYN-012 — submit must stay disabled while the passphrase is below the 12-char minimum",
    );

    await setReactInputValue("sync-enable-passphrase", PASSPHRASE);
    await setReactInputValue("sync-enable-passphrase-confirm", PASSPHRASE);
    const strengthIndicator = await $("#sync-enable-strength");
    await strengthIndicator.waitForExist({
      timeout: 5000,
    }); // SYN-012 — advisory strength indicator once the minimum is met
    await submitBtn.waitForEnabled({
      timeout: 5000,
    }); // SYN-012 satisfied, SYN-018 already satisfied (device name filled above)

    // SYN-018 — the device-name gate is independent of the passphrase gate:
    // blanking it alone must re-disable submit.
    await setReactInputValue("sync-enable-device-name", "");
    assert.strictEqual(
      await submitBtn.isEnabled(),
      false,
      "SYN-018 — submit must be disabled while the device name is blank",
    );
    await setReactInputValue("sync-enable-device-name", FIRST_DEVICE_NAME);
    await submitBtn.waitForEnabled({ timeout: 5000 });

    // -----------------------------------------------------------------
    // Step 4 — Submit reaches the real enable_sync command (SYN-011/013).
    // -----------------------------------------------------------------
    await submitBtn.click();
    // The debug build derives the Argon2id key in a few seconds before publishing.
    await modal.waitForExist({ timeout: 20000, reverse: true });
    const statusBlock = await $("#sync-status");
    await statusBlock.waitForExist({ timeout: 10000 });
    await captureScreen("sync-settings");

    // -----------------------------------------------------------------
    // Step 5 — The one thing only a real run can prove: the backend wrote
    // the encrypted folder on disk (SYN-013/050). The header file and this
    // device's manifest must exist under the real path we typed.
    // -----------------------------------------------------------------
    assert.strictEqual(
      existsSync(join(syncFolder, "vaultcompass-sync.json")),
      true,
      "SYN-013 — enable_sync must write the folder header on the real filesystem",
    );
    const devicesDir = join(syncFolder, "devices");
    assert.strictEqual(
      existsSync(devicesDir),
      true,
      "SYN-013 — enable_sync must create this device's area under devices/",
    );
    const deviceDirs = readdirSync(devicesDir);
    assert.strictEqual(
      deviceDirs.length,
      1,
      "exactly one device area must exist after the first device enables",
    );
    const [deviceDirName] = deviceDirs;
    assert.strictEqual(
      existsSync(join(devicesDir, deviceDirName, "manifest.bin")),
      true,
      "SYN-013/037 — the device's manifest must be published",
    );
    const segmentFiles = readdirSync(join(devicesDir, deviceDirName, "segments"));
    assert.ok(
      segmentFiles.some((name) => name.startsWith("seg-")),
      "SYN-013/031 — the first segment must be published alongside the manifest",
    );

    // -----------------------------------------------------------------
    // Step 6 — Status block renders the real SyncStatus (SYN-063).
    // -----------------------------------------------------------------
    const deviceNameCell = await $("#sync-status-device-name");
    assert.strictEqual(
      (await deviceNameCell.getText()).trim(),
      FIRST_DEVICE_NAME,
      "SYN-063/018 — the device name entered on enable must read back from get_sync_status",
    );
    const appVersion = await $("#sync-status-app-version");
    assert.match(
      await appVersion.getText(),
      /\d+\.\d+\.\d+/,
      "SYN-063 — the status states the version of the application running on this device",
    );
    const folderCell = await $("#sync-status-folder");
    assert.strictEqual(
      (await folderCell.getText()).trim(),
      syncFolder,
      "SYN-063 — the folder path entered on enable must read back from get_sync_status",
    );
    // SYN-070 — a freshly enabled device is never paused: "Pause" is offered, not "Resume".
    assert.strictEqual(
      await (await $("#sync-pause")).isExisting(),
      true,
      "a freshly enabled device must offer Pause, not Resume",
    );
    assert.strictEqual(
      await (await $("#sync-resume")).isExisting(),
      false,
      "a freshly enabled device must not render the Resume button",
    );

    const lastSyncCell = await $("#sync-status-last-sync");
    const lastSyncBeforeRun = (await lastSyncCell.getText()).trim();

    // -----------------------------------------------------------------
    // Step 7 — "Sync now" (SYN-061) reaches the real sync_now command: the
    // last-sync value changes from the pre-first-run placeholder to a real
    // completion timestamp — proof of the round trip, not a hard-coded string.
    // -----------------------------------------------------------------
    const syncNowBtn = await $("#sync-now");
    await syncNowBtn.waitForExist({ timeout: 8000 });
    await syncNowBtn.click();
    await syncNowBtn.waitForEnabled({ timeout: 15000 }); // re-enabled once sync_now resolved
    const lastSyncAfterRun = (await (await $("#sync-status-last-sync")).getText()).trim();
    assert.notStrictEqual(
      lastSyncAfterRun,
      lastSyncBeforeRun,
      "SYN-061/063 — sync_now must record a completion time distinct from the never-synced placeholder",
    );

    // -----------------------------------------------------------------
    // Step 8 — Pause (SYN-070): "Pause" disappears, "Resume" appears.
    // -----------------------------------------------------------------
    const pauseBtn = await $("#sync-pause");
    await pauseBtn.click();
    await pauseBtn.waitForExist({ timeout: 8000, reverse: true });
    const resumeBtn = await $("#sync-resume");
    await resumeBtn.waitForExist({ timeout: 8000 });

    // -----------------------------------------------------------------
    // Step 9 — Resume (SYN-073): reaches the real resume_sync command
    // without re-entering the passphrase; "Resume" disappears, "Pause"
    // reappears.
    // -----------------------------------------------------------------
    await resumeBtn.click();
    await resumeBtn.waitForExist({ timeout: 10000, reverse: true });
    await (await $("#sync-pause")).waitForExist({ timeout: 8000 });

    // -----------------------------------------------------------------
    // Step 10 — Rename (SYN-072): the prompt pre-fills the current name;
    // submitting reaches rename_sync_device and the new name reads back
    // from a fresh get_sync_status, not from local component state.
    // -----------------------------------------------------------------
    const renameBtn = await $("#sync-rename");
    await renameBtn.click();
    const promptDialog = await $("#sync-prompt-dialog");
    await promptDialog.waitForExist({ timeout: 8000 });
    const promptValue = await $("#sync-prompt-value");
    await promptValue.waitForExist({ timeout: 5000 });
    assert.strictEqual(
      await promptValue.getValue(),
      FIRST_DEVICE_NAME,
      "SYN-072 — the rename prompt must pre-fill the device's current name",
    );

    await setReactInputValue("sync-prompt-value", RENAMED_DEVICE_NAME);
    const promptSubmit = await $("#sync-prompt-submit");
    await promptSubmit.waitForEnabled({ timeout: 5000 });
    await promptSubmit.click();
    await promptDialog.waitForExist({ timeout: 8000, reverse: true });

    const deviceNameCellAfterRename = await $("#sync-status-device-name");
    assert.strictEqual(
      (await deviceNameCellAfterRename.getText()).trim(),
      RENAMED_DEVICE_NAME,
      "SYN-072 — the renamed device name must round-trip through rename_sync_device",
    );

    // -----------------------------------------------------------------
    // Step 11 — Leave (SYN-082): confirmation dialog, then the section
    // returns to the disabled state; the device's folder area (its
    // segment) is kept, only the manifest is removed.
    // -----------------------------------------------------------------
    const leaveBtn = await $("#sync-leave");
    await leaveBtn.click();
    const leaveConfirmBtn = await $("#sync-leave-confirm");
    await leaveConfirmBtn.waitForExist({ timeout: 8000 });
    await leaveConfirmBtn.click();

    await statusBlock.waitForExist({ timeout: 10000, reverse: true });
    await (await $("#sync-enable")).waitForExist({ timeout: 8000 });

    // Real filesystem proof: leave_sync removed the manifest but kept the
    // device's segment file (SYN-037/082) — the shared history the folder
    // holds is never abandoned by a device that leaves.
    await browser.waitUntil(
      async () => !existsSync(join(devicesDir, deviceDirName, "manifest.bin")),
      {
        timeout: 10000,
        timeoutMsg: "SYN-082 — leave_sync must remove this device's manifest from the real folder",
      },
    );
    const segmentFilesAfterLeave = readdirSync(join(devicesDir, deviceDirName, "segments"));
    assert.ok(
      segmentFilesAfterLeave.some((name) => name.startsWith("seg-")),
      "SYN-037/082 — leave_sync must keep the device's segment file; its area stays in the folder",
    );
  });
});
