import { describe, expect, it } from "vitest";
import { computePinnedScrollbar } from "./pinnedScrollbar";

// #022 — the geometry behind the holdings scrollbar: where its track starts, how far it
// travels, and whether the closed positions table can stay put.
describe("computePinnedScrollbar (#022)", () => {
  const wide = {
    viewportWidth: 1100,
    contentWidth: 1900,
    activePinnedWidth: 480,
    closedPinnedWidth: 390,
    closedWidth: 1100,
  };

  it("starts the track at the active table's pinned edge when the closed table fits", () => {
    const geometry = computePinnedScrollbar(wide);
    expect(geometry.closedStaysPut).toBe(true);
    expect(geometry.trackOffset).toBe(480);
  });

  it("gives the scrollbar the same travel as the content, over a track that excludes the pinned block", () => {
    const geometry = computePinnedScrollbar(wide);
    // 1900 − 1100 = 800 to scroll; the track is 1100 − 480 = 620 wide.
    expect(geometry.scrollRange).toBe(800);
    expect(geometry.spacerWidth).toBe(620 + 800);
    expect(geometry.visible).toBe(true);
  });

  it("lets a closed table too wide for the window scroll with the rest, from the narrower pinned edge", () => {
    const geometry = computePinnedScrollbar({ ...wide, viewportWidth: 700, closedWidth: 790 });
    expect(geometry.closedStaysPut).toBe(false);
    expect(geometry.trackOffset).toBe(390);
  });

  it("keeps the active edge when no closed table is shown", () => {
    const geometry = computePinnedScrollbar({
      ...wide,
      closedPinnedWidth: null,
      closedWidth: null,
    });
    expect(geometry.closedStaysPut).toBe(false);
    expect(geometry.trackOffset).toBe(480);
  });

  it("hides the scrollbar when every column fits", () => {
    const geometry = computePinnedScrollbar({ ...wide, contentWidth: 1100 });
    expect(geometry.scrollRange).toBe(0);
    expect(geometry.visible).toBe(false);
  });

  it("never reports a negative travel when the content is narrower than the window", () => {
    expect(computePinnedScrollbar({ ...wide, contentWidth: 900 }).scrollRange).toBe(0);
  });
});
