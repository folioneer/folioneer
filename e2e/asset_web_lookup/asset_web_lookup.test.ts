/**
 * E2E tests — Asset Web Lookup (lookup_asset)
 *
 * Contract: docs/contracts/asset-contract.md
 * Spec:     docs/spec/asset-web-lookup.md
 *
 * Network-independent tests (WEB-010, WEB-040) run in any environment.
 *
 * WEB-020/030/041/047 are covered at lower layers (Vitest + Rust integration)
 * and omitted here to avoid flaky CI failures from live OpenFIGI calls.
 */

import assert from "node:assert";
import { $ } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToAssets } from "../helpers/navigation";
import { captureScreen } from "../helpers/screenshot";

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("asset_web_lookup", () => {
  beforeEach(async () => {
    await dismissLeftoverModal();
    await navigateToAssets();
  });

  // -------------------------------------------------------------------------
  // WEB-010 — Opening the FAB shows the search modal (no network required)
  // -------------------------------------------------------------------------
  it("WEB-010: FAB opens web-lookup search modal with title visible", async () => {
    const fab = await $('button[aria-label="Add asset"]');
    await fab.waitForExist({ timeout: 10000 });
    await fab.click();

    // The modal is open when the ISIN input is present. Two-field web lookup
    // (WEB-012) renders both #web-lookup-isin-input and #web-lookup-keyword-input;
    // asserting on the ISIN input is sufficient to confirm the modal mounted.
    // Use the stable id selector — text-based selectors are unreliable in WebKitGTK.
    const searchInput = await $("#web-lookup-isin-input");
    await searchInput.waitForExist({ timeout: 8000 });
    await captureScreen("asset-web-lookup");
    assert.ok(await searchInput.isExisting(), "ISIN input must be present");

    // The "Fill manually" button must be visible immediately.
    const fillManuallyBtn = await $('button[aria-label="Fill manually"]');
    await fillManuallyBtn.waitForDisplayed({ timeout: 5000 });
    assert.ok(await fillManuallyBtn.isDisplayed(), '"Fill manually" button must be visible');
  });

  // -------------------------------------------------------------------------
  // WEB-040 — "Fill manually" skips search and opens Add Asset form (no network required)
  // -------------------------------------------------------------------------
  it("WEB-040: clicking Fill manually opens the Add Asset form without prefill", async () => {
    const fab = await $('button[aria-label="Add asset"]');
    await fab.waitForExist({ timeout: 10000 });
    await fab.click();

    // Wait for the search modal to open.
    const fillManuallyBtn = await $('button[aria-label="Fill manually"]');
    await fillManuallyBtn.waitForDisplayed({ timeout: 8000 });
    await fillManuallyBtn.click();

    // Add Asset form must appear — identified by form#add-asset-form (E2E rule E1).
    const addAssetForm = await $("form#add-asset-form");
    await addAssetForm.waitForExist({ timeout: 8000 });
    assert.ok(await addAssetForm.isExisting(), "Add asset form must appear after Fill manually");

    // The name field must be empty (no prefill in manual mode).
    const nameInput = await $("#add-asset-name");
    await nameInput.waitForExist({ timeout: 5000 });
    const nameValue = await nameInput.getValue();
    assert.strictEqual(
      nameValue,
      "",
      `Name field should be empty in manual mode, got: "${nameValue}"`,
    );

    // The Back button must NOT be rendered (back is only available in form-prefilled step).
    const backBtn = await $('button[aria-label="Back"]');
    assert.ok(
      !(await backBtn.isExisting()),
      "Back button must not be visible in manual (non-prefilled) form step",
    );
  });
});
