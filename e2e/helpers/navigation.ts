import { $ } from "@wdio/globals";

/** Navigates to the Assets page and waits for the Add Asset FAB to confirm the route is active. */
export async function navigateToAssets(): Promise<void> {
  const nav = await $("#nav-assets");
  await nav.waitForExist({ timeout: 15000 });
  await nav.click();
  const fab = await $("#fab-add-asset");
  await fab.waitForExist({ timeout: 10000 });
}

/**
 * Round-trips through the Assets page so the Accounts component remounts and
 * re-fetches, picking up any IPC-seeded accounts added after the initial load.
 * Leaves the Accounts list active (its FAB is the route-ready signal).
 */
export async function navigateToAccounts(): Promise<void> {
  const assetsNav = await $("#nav-assets");
  await assetsNav.waitForExist({ timeout: 15000 });
  await assetsNav.click();
  await $("#fab-add-asset").waitForExist({ timeout: 10000 });

  const accountsNav = await $("#nav-accounts");
  await accountsNav.waitForExist({ timeout: 10000 });
  await accountsNav.click();
  await $("#fab-add-account").waitForExist({ timeout: 10000 });
}

/**
 * Opens an account's details view from the Accounts table, selecting the row by
 * its stable `id` (E1/E4). Clicks the name cell (first column) rather than the
 * `<tr>` — the row centre overlaps action buttons that stopPropagation; the name
 * cell bubbles to the row's onClick. Assumes `navigateToAccounts()` ran first.
 * Waits for the header Performance button (PRF-010), present on every account
 * details view (live and read-only as-of).
 */
export async function navigateToAccountDetails(accountId: string): Promise<void> {
  const nameCell = await $(`#account-row-${accountId} td:first-child`);
  await nameCell.waitForExist({ timeout: 10000 });
  await nameCell.click();
  await $("#account-details-performance").waitForExist({ timeout: 10000 });
}

/**
 * Navigates to the Settings page via the sidebar nav button (`#nav-settings`),
 * waiting for the scheduled-fetch section to confirm the route rendered.
 */
export async function navigateToSettings(): Promise<void> {
  const settingsNav = await $("#nav-settings");
  await settingsNav.waitForExist({ timeout: 15000 });
  await settingsNav.click();
  const toggle = await $("#scheduled-fetch-toggle");
  await toggle.waitForExist({ timeout: 10000 });
}

/**
 * Clicks an Account Details header record action by its stable id
 * (`add-menu-open-balance`, `add-menu-dividend`, `add-menu-free-shares`). These are
 * direct big square buttons (DIV-012; the former dropdown was flattened). Cash
 * Deposit/Withdraw are NOT here — they live on the always-present cash row's inline
 * actions (CSH-019).
 */
export async function clickHeaderAction(actionId: string): Promise<void> {
  const btn = await $(`#${actionId}`);
  await btn.waitForExist({ timeout: 15000 });
  await btn.click();
}
