import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ClosedHoldingRowViewModel } from "../shared/presenter";
import { ClosedHoldingRow } from "./ClosedHoldingRow";

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en-US" },
  }),
}));

const baseRow: ClosedHoldingRowViewModel = {
  assetId: "asset-1",
  assetName: "Apple Inc",
  assetReference: "AAPL",
  realizedPnl: "150.00",
  realizedPnlRaw: 150_000_000,
  dividendsReceived: "20.00",
  dividendsReceivedRaw: 20_000_000,
  totalRevenues: "170.00",
  totalRevenuesRaw: 170_000_000,
  lastSoldDate: "2024-01-15",
};

const renderInTable = (
  row: ClosedHoldingRowViewModel,
  backfill: {
    onBackfillPriceHistory?: (assetId: string) => void;
    isBackfillingPriceHistory?: boolean;
  } = {},
) =>
  render(
    <table>
      <tbody>
        <ClosedHoldingRow row={row} accountId="account-1" {...backfill} />
      </tbody>
    </table>,
  );

describe("ClosedHoldingRow", () => {
  it("renders asset name and reference", () => {
    renderInTable(baseRow);
    expect(screen.getByText("Apple Inc")).toBeInTheDocument();
    expect(screen.getByText("AAPL")).toBeInTheDocument();
  });

  it("renders dividends received and total revenues", () => {
    renderInTable(baseRow);
    expect(screen.getByText("20.00")).toBeInTheDocument();
    expect(screen.getByText("170.00")).toBeInTheDocument();
  });

  // #006 — the closed row's view-transactions button carries a stable id and the
  // ledger icon the open rows and the header's journal button use.
  it("shows the ledger icon on its view-transactions button, under a stable id", () => {
    renderInTable(baseRow);
    const button = document.querySelector("#action-view-closed-transactions-asset-1");
    expect(button).toBeInTheDocument();
    expect(button?.getAttribute("aria-label")).toBe("transaction.list_title");
    expect(button?.querySelector("svg.lucide-scroll-text")).toBeInTheDocument();
    expect(button?.querySelector("svg.lucide-search")).toBeNull();
  });

  it("renders the lastSoldDate via formatIsoDate threaded with i18n.language", () => {
    // With language "en-US", "2024-01-15" formats to a string containing "2024" and "15".
    // The raw ISO string must NOT appear (formatIsoDate transforms it).
    renderInTable(baseRow);
    expect(screen.queryByText("2024-01-15")).not.toBeInTheDocument();
    const dateCell = screen.getByText(/2024/);
    expect(dateCell.textContent).toContain("15");
  });

  // #005 — the closed line's view-transactions action comes first, before the asset.
  it("#005 puts the action in the first cell, before the asset", () => {
    renderInTable(baseRow);
    const actionsCell = document.querySelector("tr > td:first-child");
    const assetCell = document.querySelector("tr > td:nth-child(2)");
    expect(
      actionsCell?.querySelector("#action-view-closed-transactions-asset-1"),
    ).toBeInTheDocument();
    expect(assetCell).toHaveTextContent("Apple Inc");
    expect(actionsCell?.firstElementChild).toHaveClass("grid-rows-2", "grid-flow-col");
  });

  // #019 — a closed line is set apart by muting its figures, never its actions: an
  // enabled button must look enabled.
  it("#019 leaves the Actions cell at full strength instead of dimming the whole row", () => {
    renderInTable(baseRow);
    const row = document.querySelector("tr");
    const actionsCell = document.querySelector("tr > td:first-child");
    expect(
      actionsCell?.querySelector("#action-view-closed-transactions-asset-1"),
    ).toBeInTheDocument();
    expect(row).not.toHaveClass("opacity-70");
    expect(actionsCell).not.toHaveClass("opacity-70");
    expect(actionsCell?.querySelector(".opacity-70")).toBeNull();
  });

  // #019 — a see-through sticky cell would let scrolled columns show through it, so the
  // pinned Asset cell stays opaque and only its content is muted.
  it("#019 mutes the figures and the asset text, keeping the pinned Asset cell opaque", () => {
    renderInTable(baseRow);
    const assetCell = document.querySelector("tr > td:nth-child(2)");
    expect(assetCell).toHaveTextContent("Apple Inc");
    expect(assetCell).not.toHaveClass("opacity-70");
    expect(assetCell?.firstElementChild).toHaveClass("opacity-70");
    for (const index of [3, 4, 5, 6]) {
      expect(document.querySelector(`tr > td:nth-child(${index})`)).toHaveClass("opacity-70");
    }
  });

  // #021 — jsdom has no layout: this asserts the shrink-to-fit classes; the width
  // itself is shown by the committed screenshots.
  it("#021 sizes the Asset cell to its content", () => {
    renderInTable(baseRow);
    const assetCell = document.querySelector("tr > td:nth-child(2)");
    expect(assetCell).toHaveTextContent("Apple Inc");
    expect(assetCell).toHaveClass("w-px", "whitespace-nowrap");
  });

  // #001 — jsdom has no layout: these assert the pinning classes; the scrolling
  // itself is checked on the real app by e2e/account_details/holdings_scrollbar.test.ts.
  // #001 — the closed line pins its action and asset cells like the open lines.
  it("#001 pins the action and asset cells", () => {
    renderInTable(baseRow);
    const actionsCell = document.querySelector("tr > td:first-child");
    const assetCell = document.querySelector("tr > td:nth-child(2)");
    expect(
      actionsCell?.querySelector("#action-view-closed-transactions-asset-1"),
    ).toBeInTheDocument();
    expect(actionsCell).toHaveClass("sticky", "left-0", "bg-m3-surface-container-low");
    expect(assetCell).toHaveClass("sticky", "left-[224px]", "bg-m3-surface-container-low");
    expect(document.querySelector("tr > td:nth-child(3)")).not.toHaveClass("sticky");
  });
});

describe("ClosedHoldingRow — price history backfill (MKT-190)", () => {
  it("calls the handler with the asset id from its stable-id button, after the transactions button", () => {
    const onBackfill = vi.fn();
    renderInTable(baseRow, { onBackfillPriceHistory: onBackfill });
    const button = document.querySelector("#action-backfill-closed-price-history-asset-1");
    expect(button).toBeInTheDocument();
    expect(button?.getAttribute("aria-label")).toBe("mkt.backfill.action");
    expect(button?.previousElementSibling?.id).toBe("action-view-closed-transactions-asset-1");
    fireEvent.click(button as Element);
    expect(onBackfill).toHaveBeenCalledWith("asset-1");
  });

  it("disables the button while the backfill runs", () => {
    renderInTable(baseRow, { onBackfillPriceHistory: vi.fn(), isBackfillingPriceHistory: true });
    expect(document.querySelector("#action-backfill-closed-price-history-asset-1")).toBeDisabled();
  });

  it("renders no button without a handler (read-only as-of view)", () => {
    renderInTable(baseRow);
    expect(document.querySelector("#action-backfill-closed-price-history-asset-1")).toBeNull();
  });
});
