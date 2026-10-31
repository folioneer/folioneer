/**
 * E2E tests — the import window's entry.
 *
 * Spec rules covered: IMP-060 — the window opens from an account's header, even one holding
 * nothing. The system dialog is outside the window a test can
 * drive: what follows it is covered by the hook's tests and the backend's.
 */

import assert from "node:assert";
import { $ } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import {
  clickHeaderAction,
  navigateToAccountDetails,
  navigateToAccounts,
} from "../helpers/navigation";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount } from "../helpers/seed";

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
});
