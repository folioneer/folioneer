/**
 * E2E tests — Header price item (#023, MKT-200/202/203)
 *
 * Todo entry: #023 — the header says which "sync" it is about and shows the price update
 * beside it.
 *
 * What this file proves on the real app:
 *   - the header carries the price item (MKT-202)
 *   - the date it shows comes from the backend and follows a newly recorded price without
 *     a reload (MKT-200/203)
 *
 * Seed strategy:
 *   One account holding one asset, self-contained. The price is recorded for today: no
 *   price can be newer, so the expected date does not depend on what other specs seeded
 *   into the shared database. The suite runs in en-US (wdio.conf.ts), hence M/D/YYYY.
 */

import { $, browser } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToAccounts } from "../helpers/navigation";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount, seedAsset, seedAssetPrice, seedBuy, seedCategory } from "../helpers/seed";

/** Today on this machine's clock, as ISO and as the en-US numeric date the header shows. */
function today(): { iso: string; display: string } {
  const now = new Date();
  const month = now.getMonth() + 1;
  const day = now.getDate();
  const pad = (value: number) => String(value).padStart(2, "0");
  return {
    iso: `${now.getFullYear()}-${pad(month)}-${pad(day)}`,
    display: `${month}/${day}/${now.getFullYear()}`,
  };
}

describe("price_freshness", () => {
  let assetId: string;

  before(async () => {
    const categoryId = await seedCategory("E2E Cat 023");
    const accountId = await seedAccount("E2E 023 Freshness Account");
    assetId = await seedAsset("E2E Freshness Asset", categoryId);
    await seedBuy(accountId, assetId, "2020-07-01", 5_000_000);
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  it("#023: the header's price item follows a newly recorded price", async () => {
    await navigateToAccounts();
    const item = await $("#price-freshness");
    await item.waitForDisplayed({ timeout: 10000 });

    const { iso, display } = today();
    await seedAssetPrice(assetId, iso, 12.5);

    await browser.waitUntil(async () => (await item.getText()).includes(display), {
      timeout: 10000,
      timeoutMsg: `the price item must show ${display} once a price is recorded for today`,
    });
    await captureScreen("header-price-freshness");
  });
});
