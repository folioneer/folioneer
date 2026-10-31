/**
 * E2E — MKT-215: the price source of this computer, through the real window and the real
 * commands. The suite's build starts with its no-data provider on (TODO-038), so the scenario
 * turns it off, then on again through the notice, and leaves it on for the specs after it.
 *
 * Spec rules covered: MKT-212, MKT-214, MKT-215, SPF-071.
 */
import assert from "node:assert";
import { $, browser } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { closeSettings, navigateToSettings } from "../helpers/navigation";
import { captureScreen } from "../helpers/screenshot";

describe("the price source", () => {
  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  it("turns off at once, and on only once its notice is acknowledged", async () => {
    await navigateToSettings();
    const source = await $("#settings-price-provider");
    await source.waitForExist({ timeout: 10000 });
    assert.strictEqual(await source.getValue(), "yahoo_finance");

    // Off: the fetch settings go (MKT-212, SPF-071).
    await source.selectByAttribute("value", "off");
    await (await $("#settings-auto-fetch")).waitForExist({ timeout: 10000, reverse: true });
    assert.strictEqual(await (await $("#scheduled-fetch-toggle")).isExisting(), false);
    await captureScreen("price-provider-off");

    // Choosing the provider asks first; nothing appears yet (MKT-215).
    await source.selectByAttribute("value", "yahoo_finance");
    const notice = await $("#settings-price-provider-notice");
    await notice.waitForExist({ timeout: 10000 });
    assert.strictEqual(await (await $("#settings-auto-fetch")).isExisting(), false);
    await captureScreen("price-provider-notice");

    // Acknowledged: the choice holds at once, without a restart (MKT-214).
    await (await $("#settings-price-provider-understood")).click();
    await (await $("#settings-auto-fetch")).waitForExist({ timeout: 10000 });
    await (await $("#scheduled-fetch-toggle")).waitForExist({ timeout: 10000 });
    await notice.waitForExist({ timeout: 10000, reverse: true });
    await browser.waitUntil(async () => (await source.getValue()) === "yahoo_finance", {
      timeout: 5000,
      timeoutMsg: "MKT-215 — the list must show the provider once chosen",
    });
    await captureScreen("price-provider-on");

    await closeSettings();
  });
});
