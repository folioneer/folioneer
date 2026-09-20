/**
 * E2E tests — Holdings scrollbar (#022)
 *
 * Todo entry: #022 — the holdings' horizontal scrollbar starts where the pinned columns end.
 *
 * What this file proves on the real app, where there is a layout engine (the Vitest suite
 * has none):
 *   - the scrollbar below the holdings is shown when the columns overflow, and its track
 *     starts after the pinned Actions + Asset block
 *   - moving it scrolls the content area, and scrolling the content area moves it
 *   - the pinned Asset header and a holding's first action button do not move while the
 *     other columns do
 *   - the table header stays at the top of the content area after scrolling down
 *
 * Seed strategy:
 *   One account holding eight assets. At the app's default window the thirteen columns of
 *   the active table overflow sideways and the eight rows overflow downwards, so there is
 *   something to scroll both ways. No closed position is seeded: closing one needs the
 *   sell form, which buy_sell.test.ts owns.
 */

import assert from "node:assert";
import { $, browser } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToAccountDetails, navigateToAccounts } from "../helpers/navigation";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount, seedAsset, seedBuy, seedCategory } from "../helpers/seed";

interface ScrollState {
  scrollbarLeft: number;
  areaLeft: number;
  range: number;
  trackOffset: number;
  pinnedEdge: number;
  assetHeaderLeft: number;
  quantityHeaderLeft: number;
  actionButtonLeft: number;
  headerTop: number;
  areaTop: number;
  verticalRange: number;
}

/** Reads both scroll positions and where the pinned and the first moving header sit. */
async function readScrollState(): Promise<ScrollState> {
  return (await browser.execute(() => {
    const area = document.getElementById("holdings-scroll-area") as HTMLElement;
    const scrollbar = document.getElementById("holdings-scrollbar") as HTMLElement;
    const headers = document.querySelectorAll<HTMLElement>("#holdings-table thead th");
    const assetHeader = headers.item(1);
    const quantityHeader = headers.item(2);
    const actionButton = document.querySelector<HTMLElement>(
      "#holdings-table tbody tr td:first-child button",
    ) as HTMLElement;
    const header = document.querySelector<HTMLElement>("#holdings-table thead") as HTMLElement;
    const areaLeft = area.getBoundingClientRect().left;
    return {
      scrollbarLeft: scrollbar.scrollLeft,
      areaLeft: area.scrollLeft,
      range: area.scrollWidth - area.clientWidth,
      trackOffset: scrollbar.getBoundingClientRect().left - areaLeft,
      pinnedEdge: assetHeader.getBoundingClientRect().right - areaLeft,
      assetHeaderLeft: assetHeader.getBoundingClientRect().left,
      quantityHeaderLeft: quantityHeader.getBoundingClientRect().left,
      actionButtonLeft: actionButton.getBoundingClientRect().left,
      headerTop: header.getBoundingClientRect().top,
      areaTop: area.getBoundingClientRect().top,
      verticalRange: area.scrollHeight - area.clientHeight,
    };
  })) as ScrollState;
}

describe("holdings_scrollbar", () => {
  let accountId: string;

  before(async () => {
    const categoryId = await seedCategory("E2E Cat 022");
    accountId = await seedAccount("E2E 022 Scrollbar Account");
    for (let index = 1; index <= 8; index += 1) {
      const assetId = await seedAsset(`E2E Scrollbar Asset ${index}`, categoryId);
      await seedBuy(accountId, assetId, "2020-07-01", 5_000_000);
    }
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  it("#022: the scrollbar starts after the pinned columns and scrolls the holdings with it", async () => {
    await navigateToAccounts();
    await navigateToAccountDetails(accountId);

    const scrollbar = await $("#holdings-scrollbar");
    await scrollbar.waitForDisplayed({ timeout: 10000 });

    const atRest = await readScrollState();
    assert.ok(
      atRest.range > 0,
      `the columns must overflow for this scenario, range ${atRest.range}`,
    );
    assert.ok(
      Math.abs(atRest.trackOffset - atRest.pinnedEdge) <= 1,
      `the track must start at the pinned edge: track ${atRest.trackOffset}, pinned ${atRest.pinnedEdge}`,
    );

    // The scrollbar drives the content area.
    const target = Math.min(120, atRest.range);
    await browser.execute((left: number) => {
      (document.getElementById("holdings-scrollbar") as HTMLElement).scrollLeft = left;
    }, target);
    await browser.waitUntil(async () => (await readScrollState()).areaLeft === target, {
      timeout: 5000,
      timeoutMsg: `the content area must follow the scrollbar to ${target}`,
    });

    const scrolled = await readScrollState();
    // Positions can be fractional, so both comparisons allow one pixel.
    assert.ok(
      Math.abs(scrolled.assetHeaderLeft - atRest.assetHeaderLeft) <= 1,
      `the pinned Asset header must stay put: ${atRest.assetHeaderLeft} → ${scrolled.assetHeaderLeft}`,
    );
    assert.ok(
      Math.abs(scrolled.actionButtonLeft - atRest.actionButtonLeft) <= 1,
      `a holding's action button must stay put: ${atRest.actionButtonLeft} → ${scrolled.actionButtonLeft}`,
    );
    assert.ok(
      Math.abs(scrolled.quantityHeaderLeft - (atRest.quantityHeaderLeft - target)) <= 1,
      `the first moving column must scroll by ${target}: ${atRest.quantityHeaderLeft} → ${scrolled.quantityHeaderLeft}`,
    );
    await captureScreen("account-details-holdings-scrolled");

    // The content area drives the scrollbar — what the keyboard and the trackpad do.
    await browser.execute(() => {
      (document.getElementById("holdings-scroll-area") as HTMLElement).scrollLeft = 0;
    });
    await browser.waitUntil(async () => (await readScrollState()).scrollbarLeft === 0, {
      timeout: 5000,
      timeoutMsg: "the scrollbar must follow the content area back to 0",
    });
  });

  it("#001: the table header stays at the top of the content area after scrolling down", async () => {
    await navigateToAccounts();
    await navigateToAccountDetails(accountId);
    const table = await $("#holdings-table");
    await table.waitForDisplayed({ timeout: 10000 });

    const atRest = await readScrollState();
    assert.ok(
      atRest.verticalRange > 0,
      `the rows must overflow downwards for this scenario, range ${atRest.verticalRange}`,
    );

    const target = Math.min(150, atRest.verticalRange);
    await browser.execute((top: number) => {
      (document.getElementById("holdings-scroll-area") as HTMLElement).scrollTop = top;
    }, target);
    await browser.waitUntil(
      async () =>
        (await browser.execute(
          () => (document.getElementById("holdings-scroll-area") as HTMLElement).scrollTop,
        )) === target,
      { timeout: 5000, timeoutMsg: `the content area must scroll down to ${target}` },
    );

    const scrolled = await readScrollState();
    assert.ok(
      Math.abs(scrolled.headerTop - scrolled.areaTop) <= 1,
      `the header must sit at the top of the content area: header ${scrolled.headerTop}, area ${scrolled.areaTop}`,
    );
  });
});
