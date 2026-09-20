/**
 * E2E tests — Asset exchange field (AST-021, AST-022)
 *
 * Contract: docs/contracts/asset-contract.md
 * Spec rules covered:
 *   AST-021 — add_asset carries exchange: Option<Exchange>; user picks from MIC list
 *   AST-022 — update_asset: user can freely set, change, or clear exchange
 *
 * Coverage strategy (pyramid):
 *   Unit + integration tests cover exchange validation (InvalidExchange), DTO
 *   mapping, and repository round-trips. These three scenarios exercise the
 *   critical-path UI → IPC → backend handshakes that only surface end-to-end:
 *     1. Add Asset with an explicit exchange pick → exchange persisted and
 *        visible in the Edit form after creation.
 *     2. Edit Asset — change exchange via picker → picker reflects new value.
 *     3. Edit Asset — clear exchange via "(none)" → picker shows no exchange.
 *
 * Selector strategy (E1–E4):
 *   - form#add-asset-form / form#edit-asset-form         (E1)
 *   - #add-asset-exchange-picker / #edit-asset-exchange-picker (E2, via idPrefix)
 *   - button[type="submit"][form="add-asset-form"]        (E3)
 *   - button[id^="action-edit-asset-"] scoped to asset row (locale-invariant; per-row id)
 */

import assert from "node:assert";
import { $, browser } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToAssets } from "../helpers/navigation";
import { setReactInputValue } from "../helpers/react";
import { captureScreen } from "../helpers/screenshot";
import { seedAsset, seedCategory } from "../helpers/seed";

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("asset_exchange", () => {
  // Shared category — seeded once; individual tests add their own assets to
  // keep names unique and runs independent.
  let sharedCategoryId: string;

  before(async () => {
    sharedCategoryId = await seedCategory("E2E Exchange Cat");
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
    await navigateToAssets();
  });

  // -------------------------------------------------------------------------
  // AST-021 — Add Asset with explicit exchange pick
  // -------------------------------------------------------------------------
  it("AST-021: add_asset persists selected exchange; exchange visible in edit form", async () => {
    const ASSET_NAME = "E2E Exchange Add XPAR";

    // Open the Add Asset flow via the FAB.
    const fab = await $("#fab-add-asset");
    await fab.click();

    // WebLookupModal opens first — skip to the manual form.
    const fillManually = await $('button[aria-label="Fill manually"]');
    await fillManually.waitForDisplayed({ timeout: 8000 });
    await fillManually.click();

    const form = await $("form#add-asset-form");
    await form.waitForExist({ timeout: 8000 });

    // Fill the required fields.
    await setReactInputValue("add-asset-name", ASSET_NAME);
    await setReactInputValue("add-asset-reference", "XPRADD");
    await setReactInputValue("add-asset-currency", "EUR");

    const categorySelect = await $("#add-asset-category");
    await categorySelect.waitForExist({ timeout: 5000 });
    await categorySelect.selectByVisibleText("E2E Exchange Cat");

    // Select exchange from the ExchangePicker (native <select>, value = MIC code).
    const exchangePicker = await $("#add-asset-exchange-picker");
    await exchangePicker.waitForExist({ timeout: 5000 });
    // XPAR (Euronext Paris) is in the curated set per AST-021.
    await exchangePicker.selectByAttribute("value", "XPAR");

    const submitBtn = await $('button[type="submit"][form="add-asset-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    // Form closes on success.
    await form.waitForExist({ timeout: 8000, reverse: true });

    // Navigate away and back to defeat any stale loading state from the
    // AssetUpdated event re-fetch (same pattern as assets.test.ts).
    const accountsNav = await $("#nav-accounts");
    await accountsNav.waitForExist({ timeout: 10000 });
    await accountsNav.click();
    await $("#fab-add-account").waitForExist({ timeout: 10000 });
    await navigateToAssets();

    // Open the Edit form for the newly created asset to verify exchange persisted.
    const editBtn = await $(
      `//tr[.//td[normalize-space(text())="${ASSET_NAME}"]]//button[starts-with(@id, "action-edit-asset-")]`,
    );
    await editBtn.waitForExist({ timeout: 10000 });
    await editBtn.click();

    const editForm = await $("form#edit-asset-form");
    await editForm.waitForExist({ timeout: 8000 });

    const editExchangePicker = await $("#edit-asset-exchange-picker");
    await editExchangePicker.waitForExist({ timeout: 5000 });
    await captureScreen("asset-edit-modal");
    const persistedCode = await editExchangePicker.getValue();
    assert.strictEqual(
      persistedCode,
      "XPAR",
      `Exchange picker must show "XPAR" after add_asset with XPAR; got "${persistedCode}"`,
    );
  });

  // -------------------------------------------------------------------------
  // AST-022 — Edit Asset: change exchange to a different value
  // -------------------------------------------------------------------------
  it("AST-022 (change): update_asset persists new exchange when changed via picker", async () => {
    const ASSET_NAME = "E2E Exchange Change";

    // Seed an asset with XPAR already set via a direct update_asset IPC call
    // after the initial seedAsset (which creates with exchange: null).
    const assetId = await seedAsset(ASSET_NAME, sharedCategoryId, {
      reference: "XPRCNG",
    });

    // Set exchange to XPAR via IPC so we can assert a change to XNAS.
    await browser.executeAsync(
      (id: string, catId: string, done: (r: unknown) => void) => {
        // @ts-expect-error __TAURI_INTERNALS__ injected by Tauri WebView
        window.__TAURI_INTERNALS__
          .invoke("update_asset", {
            dto: {
              asset_id: id,
              name: "E2E Exchange Change",
              reference: "XPRCNG",
              class: "Stocks",
              currency: "EUR",
              risk_level: 3,
              category_id: catId,
              interest_bearing: false,
              exchange: { code: "XPAR", label: "Euronext Paris" },
            },
          })
          .then(done)
          .catch((err: unknown) => done({ __error: String(err) }));
      },
      assetId,
      sharedCategoryId,
    );

    await navigateToAssets();

    // Open Edit for the seeded asset.
    const editBtn = await $(
      `//tr[.//td[normalize-space(text())="${ASSET_NAME}"]]//button[starts-with(@id, "action-edit-asset-")]`,
    );
    await editBtn.waitForExist({ timeout: 10000 });
    await editBtn.click();

    const editForm = await $("form#edit-asset-form");
    await editForm.waitForExist({ timeout: 8000 });

    // Confirm picker starts on XPAR.
    const exchangePicker = await $("#edit-asset-exchange-picker");
    await exchangePicker.waitForExist({ timeout: 5000 });
    const initialCode = await exchangePicker.getValue();
    assert.strictEqual(
      initialCode,
      "XPAR",
      `Exchange picker must start at "XPAR" before change; got "${initialCode}"`,
    );

    // Change to XNAS (NASDAQ).
    await exchangePicker.selectByAttribute("value", "XNAS");

    const submitBtn = await $('button[type="submit"][form="edit-asset-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    // Form closes on success.
    await editForm.waitForExist({ timeout: 8000, reverse: true });

    // Reopen Edit and verify the new exchange is persisted.
    const editBtnAfter = await $(
      `//tr[.//td[normalize-space(text())="${ASSET_NAME}"]]//button[starts-with(@id, "action-edit-asset-")]`,
    );
    await editBtnAfter.waitForExist({ timeout: 10000 });
    await editBtnAfter.click();

    const editFormAfter = await $("form#edit-asset-form");
    await editFormAfter.waitForExist({ timeout: 8000 });

    const exchangePickerAfter = await $("#edit-asset-exchange-picker");
    await exchangePickerAfter.waitForExist({ timeout: 5000 });
    const updatedCode = await exchangePickerAfter.getValue();
    assert.strictEqual(
      updatedCode,
      "XNAS",
      `Exchange picker must show "XNAS" after update_asset change; got "${updatedCode}"`,
    );
  });

  // -------------------------------------------------------------------------
  // AST-022 — Edit Asset: clear exchange (set to none)
  // -------------------------------------------------------------------------
  it("AST-022 (clear): update_asset clears exchange when picker set to none", async () => {
    const ASSET_NAME = "E2E Exchange Clear";

    // Seed an asset, then set its exchange to XPAR via IPC.
    const assetId = await seedAsset(ASSET_NAME, sharedCategoryId, {
      reference: "XPRCLR",
    });

    await browser.executeAsync(
      (id: string, catId: string, done: (r: unknown) => void) => {
        // @ts-expect-error __TAURI_INTERNALS__ injected by Tauri WebView
        window.__TAURI_INTERNALS__
          .invoke("update_asset", {
            dto: {
              asset_id: id,
              name: "E2E Exchange Clear",
              reference: "XPRCLR",
              class: "Stocks",
              currency: "EUR",
              risk_level: 3,
              category_id: catId,
              interest_bearing: false,
              exchange: { code: "XPAR", label: "Euronext Paris" },
            },
          })
          .then(done)
          .catch((err: unknown) => done({ __error: String(err) }));
      },
      assetId,
      sharedCategoryId,
    );

    await navigateToAssets();

    // Open Edit.
    const editBtn = await $(
      `//tr[.//td[normalize-space(text())="${ASSET_NAME}"]]//button[starts-with(@id, "action-edit-asset-")]`,
    );
    await editBtn.waitForExist({ timeout: 10000 });
    await editBtn.click();

    const editForm = await $("form#edit-asset-form");
    await editForm.waitForExist({ timeout: 8000 });

    // Confirm the picker starts at XPAR.
    const exchangePicker = await $("#edit-asset-exchange-picker");
    await exchangePicker.waitForExist({ timeout: 5000 });
    const initialCode = await exchangePicker.getValue();
    assert.strictEqual(
      initialCode,
      "XPAR",
      `Exchange picker must start at "XPAR" before clear; got "${initialCode}"`,
    );

    // Select the "(none)" option — value="" clears the exchange.
    await exchangePicker.selectByAttribute("value", "");

    const submitBtn = await $('button[type="submit"][form="edit-asset-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    // Form closes on success.
    await editForm.waitForExist({ timeout: 8000, reverse: true });

    // Reopen Edit and verify exchange is cleared (picker value = "").
    const editBtnAfter = await $(
      `//tr[.//td[normalize-space(text())="${ASSET_NAME}"]]//button[starts-with(@id, "action-edit-asset-")]`,
    );
    await editBtnAfter.waitForExist({ timeout: 10000 });
    await editBtnAfter.click();

    const editFormAfter = await $("form#edit-asset-form");
    await editFormAfter.waitForExist({ timeout: 8000 });

    const exchangePickerAfter = await $("#edit-asset-exchange-picker");
    await exchangePickerAfter.waitForExist({ timeout: 5000 });
    const clearedCode = await exchangePickerAfter.getValue();
    assert.strictEqual(
      clearedCode,
      "",
      `Exchange picker must show "" (none) after clearing exchange; got "${clearedCode}"`,
    );
  });
});
