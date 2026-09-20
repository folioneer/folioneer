/**
 * E2E tests — Holding Note flow (HNO)
 *
 * Spec:     docs/spec/holding-note.md
 * Contract: docs/contracts/account-contract.md § upsert_holding_note / delete_holding_note
 *
 * Spec rules covered by this file:
 *   HNO-042 — "Note" holding-row action opens the note modal (textarea + alarm)
 *   HNO-020 — saving upserts the note for the (account, asset) pair
 *   HNO-030 — the alarm triggers statelessly from the current price
 *   HNO-041 — the row renders the note text + a triggered (filled) bell
 *   HNO-021 — deleting removes the note; the row line disappears
 *
 * Seed strategy:
 *   - category + asset (Stocks, non-cash) + account + buy 10 units @ 100 via IPC
 *     (seedBuy internally seeds a deposit — CSH-041 pre-condition).
 *   - An asset price of 120 is seeded so the "Above 100" alarm recorded by the
 *     scenario is already crossed (120 > 100, strict comparison per HNO-030) —
 *     the bell must render in its triggered state.
 *   - The textarea is a React controlled <textarea> driven with
 *     setReactTextareaValue; the threshold is a CalcField (type="text") driven
 *     with setReactInputValue (E6); the direction is a native <select> driven
 *     with selectByAttribute (locale-invariant).
 *
 * Why one scenario:
 *   The critical path (save → note line + triggered bell → delete → gone)
 *   exercises the full UI → IPC → read stack. Validation variants
 *   (NoteTextEmpty, ThresholdNotPositive, NoteOnCashAsset, …) are covered at
 *   the backend-integration and Vitest-frontend tiers.
 */

import { $, browser } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToAccountDetails, navigateToAccounts } from "../helpers/navigation";
import { setReactInputValue, setReactTextareaValue } from "../helpers/react";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount, seedAsset, seedAssetPrice, seedBuy, seedCategory } from "../helpers/seed";

// ---------------------------------------------------------------------------
// Fixed values (E2E rule E9)
// ---------------------------------------------------------------------------
const NOTE_TEXT = "Buy 5 more when it crosses 100";

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("holding_note", () => {
  const ACCOUNT_NAME = "E2E Note HNO-020";
  const ASSET_NAME = "E2E Asset HNO020";
  let astId: string;
  let accId: string;

  // Seed prerequisites once via IPC — no UI interaction for setup.
  // A 10-unit buy @ 100 gives the pair holding history (HNO-011); the price
  // of 120 arms-and-triggers the "Above 100" alarm the scenario records.
  before(async () => {
    const catId = await seedCategory("E2E Cat HNO020");
    accId = await seedAccount(ACCOUNT_NAME);
    astId = await seedAsset(ASSET_NAME, catId);
    await seedBuy(accId, astId, "2020-09-01", 10_000_000); // 10 units in micros
    await seedAssetPrice(astId, "2020-10-01", 120);
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  // -------------------------------------------------------------------------
  // HNO-042/020/030/041/021 — self-cleaning critical path (one scenario, like
  //   free_shares.test.ts): open the note modal from the holding row → save a
  //   note with an Above-100 alarm (current price 120 → triggered) → the row
  //   carries the note text and the triggered bell → reopen prefilled →
  //   delete → the line disappears.
  // -------------------------------------------------------------------------
  it("HNO-042/020/041/021: note round-trip — save with triggered alarm, row shows it, delete removes it", async () => {
    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    // -------------------------------------------------------------------
    // Step 1 — Open the note modal from the holding-row action (HNO-042).
    // -------------------------------------------------------------------
    const noteBtn = await $(`#action-note-${astId}`);
    await noteBtn.waitForExist({ timeout: 8000 });
    await noteBtn.click();

    // -------------------------------------------------------------------
    // Step 2 — Wait for the note form (HNO-042).
    // -------------------------------------------------------------------
    const form = await $("form#holding-note-form");
    await form.waitForExist({ timeout: 8000 });

    // -------------------------------------------------------------------
    // Step 3 — Fill the form.
    //   text: controlled <textarea> → setReactTextareaValue (E6).
    //   alarm: checkbox click reveals direction + threshold (HNO-042).
    //   direction: native <select> → selectByAttribute (locale-invariant).
    //   threshold: CalcField (type="text") → setReactInputValue (E6);
    //   100 < the seeded price 120, so "Above" is already crossed (HNO-030).
    // -------------------------------------------------------------------
    await setReactTextareaValue("holding-note-text", NOTE_TEXT);

    const alarmToggle = await $("#holding-note-alarm-toggle");
    await alarmToggle.waitForExist({ timeout: 5000 });
    await alarmToggle.click();

    const directionSelect = await $("#holding-note-direction");
    await directionSelect.waitForExist({ timeout: 5000 });
    await directionSelect.selectByAttribute("value", "Above");

    await setReactInputValue("holding-note-price", "100");

    // -------------------------------------------------------------------
    // Step 4 — Submit (HNO-020 valid path).
    //   waitForEnabled confirms React processed the input events (E6).
    // -------------------------------------------------------------------
    const submitBtn = await $('button[type="submit"][form="holding-note-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    // The form must close on success (HNO-022 refresh follows).
    await form.waitForExist({ timeout: 8000, reverse: true });

    // -------------------------------------------------------------------
    // Step 5 — Row rendering (HNO-041): the note line carries the full text
    //   in its `title` (the tooltip) and the bell renders (triggered:
    //   120 > 100, HNO-030). The title is asserted instead of getText():
    //   the truncated single-line span can compute to zero width under the
    //   headless CI font metrics, and WebDriver reports invisible text as "".
    // -------------------------------------------------------------------
    const noteLine = await $(`#holding-note-${astId}`);
    await noteLine.waitForExist({ timeout: 8000 });
    await browser.waitUntil(async () => (await noteLine.getAttribute("title")) === NOTE_TEXT, {
      timeout: 8000,
      timeoutMsg: "The holding row must carry the saved note text (HNO-041)",
    });

    const bell = await $(`#holding-note-bell-${astId}`);
    await bell.waitForExist({ timeout: 8000 });
    await captureScreen("holding-note-alarm");

    // -------------------------------------------------------------------
    // Step 6 — Reopen the modal; the stored note marks edit mode, so the
    //   destructive Delete action is offered (HNO-042). Delete (HNO-021).
    // -------------------------------------------------------------------
    await noteBtn.click();
    await form.waitForExist({ timeout: 8000 });

    const deleteBtn = await $("#holding-note-delete");
    await deleteBtn.waitForExist({ timeout: 8000 });
    await deleteBtn.click();

    // HNO-021 — the destructive action asks for confirmation first.
    const confirmBtn = await $("#holding-note-delete-confirm");
    await confirmBtn.waitForExist({ timeout: 5000 });
    await confirmBtn.click();

    // The form must close on success.
    await form.waitForExist({ timeout: 8000, reverse: true });

    // -------------------------------------------------------------------
    // Step 7 — The note line and its bell are gone (HNO-041: no note →
    //   nothing renders) — the scenario leaves no state behind.
    // -------------------------------------------------------------------
    await noteLine.waitForExist({ timeout: 8000, reverse: true });
    await bell.waitForExist({ timeout: 2000, reverse: true });
  });
});
