/**
 * E2E tests — the import window's entry.
 *
 * Spec rules covered: IMP-060 — the window opens from an account's header, even one holding
 * nothing. IMP-066 to IMP-068, IMP-054 — a fixture file (the run names it, the system dialog
 * being outside what a test can drive) previewed, an unknown asset matched, the rows recorded,
 * the added deposit marked in the journal.
 */

import assert from "node:assert";
import { $ } from "@wdio/globals";
import { dismissLeftoverModal, waitForSnackbarGone } from "../helpers/modal";
import {
  clickHeaderAction,
  navigateToAccountDetails,
  navigateToAccounts,
} from "../helpers/navigation";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount, seedAsset, seedCategory, seedImportMapper } from "../helpers/seed";

describe("account_import", () => {
  let accountId: string;

  before(async () => {
    accountId = await seedAccount("E2E Import Account", "EUR");
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  it("IMP-060: an empty account's header opens the import window, and Cancel closes it", async () => {
    await navigateToAccounts();
    await navigateToAccountDetails(accountId);
    await clickHeaderAction("add-menu-import");

    const choose = await $("#account-import-choose-file");
    await choose.waitForExist({ timeout: 8000 });
    await captureScreen("account-import-pick-file");
    assert.ok(await choose.isEnabled(), "the file can be chosen at once");

    await (await $("#account-import-cancel")).click();
    await choose.waitForExist({ timeout: 5000, reverse: true });
  });

  it("IMP-066: a file is previewed, an unknown asset matched, its rows recorded", async () => {
    const categoryId = await seedCategory("E2E Import Category");
    await seedAsset("E2E Import Asset", categoryId);
    const matchedId = await seedAsset("E2E Matched Asset", categoryId);
    const mapperId = await seedImportMapper("E2E Broker");

    await navigateToAccounts();
    await navigateToAccountDetails(accountId);
    await clickHeaderAction("add-menu-import");
    await (await $("#account-import-choose-file")).click();

    const mapper = await $("#account-import-mapper");
    await mapper.waitForExist({ timeout: 8000 });
    await mapper.selectByAttribute("value", mapperId);
    await (await $("#account-import-matches")).waitForExist({ timeout: 8000 });
    await (await $("#account-import-preview")).click();

    const unknown = await $("#account-import-unknown-5");
    await unknown.waitForExist({ timeout: 8000 });
    assert.ok(await (await $("#account-import-count-rejected")).isExisting());
    assert.ok(await (await $("#account-import-count-not-imported")).isExisting());
    assert.ok(await (await $("#account-import-note-4")).isExisting(), "the deposit it needs");
    assert.ok(await (await $("#account-import-add-deposits")).isSelected());
    await captureScreen("account-import-preview");

    await unknown.click();
    const asset = await $("#account-import-match-asset");
    await asset.waitForExist({ timeout: 5000 });
    await asset.selectByAttribute("value", matchedId);
    await (await $("#account-import-match")).click();
    await unknown.waitForExist({ timeout: 8000, reverse: true });

    await (await $("#account-import-record")).click();
    const modal = await $("#account-import-rows");
    await modal.waitForExist({ timeout: 8000, reverse: true });

    await waitForSnackbarGone();
    await (await $("#account-details-journal")).click();
    await (await $('[id^="txl-added-deposit-chip-"]')).waitForExist({ timeout: 8000 });
    await captureScreen("account-import-journal");
  });
});
