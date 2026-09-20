/**
 * E2E tests — Account management (CRUD)
 *
 * Contract: docs/contracts/account-contract.md
 * Spec rules covered:
 *   ACC-001 — create account → appears in account list
 *   ACC-002 — edit account name → list reflects update
 *   ACC-003 — delete account → removed from list
 *   ACC-004 — create duplicate name → inline error shown
 */

import assert from "node:assert";
import { $ } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { setReactInputValue } from "../helpers/react";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount } from "../helpers/seed";

// ---------------------------------------------------------------------------
// Navigation helpers
// ---------------------------------------------------------------------------

async function navigateToAccounts(): Promise<void> {
  const nav = await $("#nav-accounts");
  await nav.waitForExist({ timeout: 15000 });
  await nav.click();
  const fab = await $("#fab-add-account");
  await fab.waitForExist({ timeout: 10000 });
}

// Navigate away (to Assets) then back to Accounts, forcing the accounts
// component to remount and re-fetch from the store. Use this after IPC
// seeding when the accounts page is already mounted.
async function forceRefreshToAccounts(): Promise<void> {
  const assetsNav = await $("#nav-assets");
  await assetsNav.waitForExist({ timeout: 15000 });
  await assetsNav.click();
  await $("#fab-add-asset").waitForExist({ timeout: 10000 });
  await navigateToAccounts();
}

// Language-invariant selector: finds the account row by the tr aria-label
// ("Open account NAME") set by AccountTable — the account name is user data
// and is never translated.
async function findAccountRow(accountName: string) {
  return $(`tr[aria-label="Open account ${accountName}"]`);
}

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("accounts", () => {
  beforeEach(async () => {
    await dismissLeftoverModal();
    await navigateToAccounts();
  });

  // -------------------------------------------------------------------------
  // ACC-001 — create account → appears in account list
  // -------------------------------------------------------------------------
  it("ACC-001: creating an account shows it in the account list", async () => {
    const ACCOUNT_NAME = "E2E ACC-001 Account";

    const fab = await $("#fab-add-account");
    await fab.click();

    const form = await $("form#add-account-form");
    await form.waitForExist({ timeout: 8000 });

    await setReactInputValue("add-account-name", ACCOUNT_NAME);
    await setReactInputValue("add-account-currency", "EUR");

    const submitBtn = await $('button[type="submit"][form="add-account-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    await form.waitForExist({ timeout: 8000, reverse: true });

    // After creation the app navigates to the new account's detail page.
    // Navigate back to the accounts list to verify the entry is there.
    await navigateToAccounts();

    const accountRow = await findAccountRow(ACCOUNT_NAME);
    await accountRow.waitForExist({ timeout: 10000 });
    await captureScreen("accounts-list");
    assert.ok(
      await accountRow.isExisting(),
      `Account "${ACCOUNT_NAME}" must appear in list after creation`,
    );
  });

  // -------------------------------------------------------------------------
  // ACC-002 — edit account name → list reflects update
  // -------------------------------------------------------------------------
  it("ACC-002: editing an account name updates the list", async () => {
    const ORIGINAL_NAME = "E2E ACC-002 Original";
    const UPDATED_NAME = "E2E ACC-002 Updated";

    const editAccountId = await seedAccount(ORIGINAL_NAME);

    // Navigate away and back to force the accounts list to remount and pick
    // up the IPC-seeded account from the store.
    await forceRefreshToAccounts();

    const editBtn = await $(`#action-edit-account-${editAccountId}`);
    await editBtn.waitForExist({ timeout: 8000 });
    await editBtn.click();

    const form = await $("form#edit-account-form");
    await form.waitForExist({ timeout: 8000 });

    await setReactInputValue("edit-account-name", UPDATED_NAME);

    const submitBtn = await $('button[type="submit"][form="edit-account-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    await form.waitForExist({ timeout: 8000, reverse: true });

    const updatedRow = await findAccountRow(UPDATED_NAME);
    await updatedRow.waitForExist({ timeout: 10000 });
    assert.ok(
      await updatedRow.isExisting(),
      `Updated account "${UPDATED_NAME}" must appear in list`,
    );
  });

  // -------------------------------------------------------------------------
  // ACC-003 — delete account → removed from list
  // -------------------------------------------------------------------------
  it("ACC-003: deleting an account removes it from the list", async () => {
    const ACCOUNT_NAME = "E2E ACC-003 Delete";

    const deleteAccountId = await seedAccount(ACCOUNT_NAME);

    // Force remount so the seeded account appears in the list.
    await forceRefreshToAccounts();

    const accountRow = await findAccountRow(ACCOUNT_NAME);
    await accountRow.waitForExist({ timeout: 8000 });

    const deleteBtn = await $(`#action-delete-account-${deleteAccountId}`);
    await deleteBtn.waitForExist({ timeout: 5000 });
    await deleteBtn.click();

    // Confirm in the dialog (confirmLabel = "Delete") — scoped to dialog to avoid
    // matching the row-level Delete button that remains in the DOM.
    const confirmBtn = await $('//*[@role="dialog"]//button[normalize-space()="Delete"]');
    await confirmBtn.waitForDisplayed({ timeout: 5000 });
    await confirmBtn.click();

    await accountRow.waitForExist({ timeout: 8000, reverse: true });
    assert.ok(
      !(await accountRow.isExisting()),
      `Account "${ACCOUNT_NAME}" must be removed from list after deletion`,
    );
  });

  // -------------------------------------------------------------------------
  // ACC-004 — duplicate name → inline error shown
  // -------------------------------------------------------------------------
  it("ACC-004: creating an account with a duplicate name shows an inline error", async () => {
    const DUPLICATE_NAME = "E2E ACC-004 Duplicate";

    await seedAccount(DUPLICATE_NAME);
    await forceRefreshToAccounts();

    const fab = await $("#fab-add-account");
    await fab.click();

    const form = await $("form#add-account-form");
    await form.waitForExist({ timeout: 8000 });

    await setReactInputValue("add-account-name", DUPLICATE_NAME);
    await setReactInputValue("add-account-currency", "EUR");

    const submitBtn = await $('button[type="submit"][form="add-account-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    const alert = await $('[role="alert"]');
    await alert.waitForExist({ timeout: 8000 });
    assert.ok(await alert.isExisting(), "Inline error must appear for duplicate account name");
    assert.ok(await form.isExisting(), "Modal must remain open after duplicate name error");
  });
});
