import { $ } from "@wdio/globals";

describe("smoke", () => {
  it("app launches and shell is visible", async () => {
    // The sidebar is always rendered on initial load regardless of route or
    // update availability. The "Accounts" nav button is a BASE_NAV_ITEM
    // (never gated by import.meta.env.DEV) with aria-label driven by
    // t("nav.accounts") → "Accounts" in en/common.json.
    const accountsNav = await $("#nav-accounts");
    await accountsNav.waitForExist({ timeout: 15000 });
  });
});
