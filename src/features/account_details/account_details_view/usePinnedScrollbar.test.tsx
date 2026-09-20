import { fireEvent, render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { usePinnedScrollbar } from "./usePinnedScrollbar";

function Harness({ closedSection = "none" }: { closedSection?: "none" | "folded" }) {
  const { scrollAreaRef, scrollbarRef, scrollbar } = usePinnedScrollbar();
  return (
    <div>
      <div id="area" ref={scrollAreaRef}>
        <div>
          <table id="holdings-table">
            <thead>
              <tr>
                <th>Actions</th>
                <th>Asset</th>
              </tr>
            </thead>
          </table>
          {closedSection === "folded" && (
            <div id="closed-holdings-section">
              <button type="button">Closed positions</button>
            </div>
          )}
        </div>
      </div>
      <div id="bar" ref={scrollbarRef} hidden={!scrollbar.visible} />
      <output id="closed-stays-put">{String(scrollbar.closedStaysPut)}</output>
    </div>
  );
}

// #022 — the scrollbar and the content area stay scrolled together, whichever moves first.
describe("usePinnedScrollbar (#022)", () => {
  it("scrolls the content area when the scrollbar moves", () => {
    render(<Harness />);
    const area = document.getElementById("area") as HTMLElement;
    const bar = document.getElementById("bar") as HTMLElement;
    bar.scrollLeft = 240;
    fireEvent.scroll(bar);
    expect(area.scrollLeft).toBe(240);
  });

  it("moves the scrollbar when the content area is scrolled by the keyboard or the trackpad", () => {
    render(<Harness />);
    const area = document.getElementById("area") as HTMLElement;
    const bar = document.getElementById("bar") as HTMLElement;
    area.scrollLeft = 130;
    fireEvent.scroll(area);
    expect(bar.scrollLeft).toBe(130);
  });

  // A folded closed section has no table, but its header bar is still there and fits.
  it("keeps a folded closed section put, like an open one that fits", () => {
    render(<Harness closedSection="folded" />);
    expect(document.getElementById("closed-stays-put")).toHaveTextContent("true");
  });

  it("reports nothing to keep put when the account has no closed position", () => {
    render(<Harness />);
    expect(document.getElementById("closed-stays-put")).toHaveTextContent("false");
  });

  it("stays hidden while there is nothing to scroll", () => {
    render(<Harness />);
    expect(document.getElementById("bar")).toHaveAttribute("hidden");
  });
});
