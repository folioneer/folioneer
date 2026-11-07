import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { HoldingLotsModal } from "./HoldingLotsModal";

const mockGetHoldingLots = vi.fn();

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    getHoldingLots: (...args: unknown[]) => mockGetHoldingLots(...args),
  },
}));

const target = {
  assetId: "asset-1",
  assetName: "Acme",
  quantity: "4",
  costBasis: "400.00",
  currency: "EUR",
};

describe("HoldingLotsModal (#515, the validated mock)", () => {
  // While the lots are read the dialog says so; with none it says so too.
  it("shows the read, then an empty state", async () => {
    let resolve: (value: unknown) => void = () => undefined;
    mockGetHoldingLots.mockReturnValue(new Promise((r) => (resolve = r)));
    render(<HoldingLotsModal isOpen onClose={vi.fn()} accountId="account-1" target={target} />);
    expect(document.getElementById("holding-lots-loading")).not.toBeNull();
    resolve({ status: "ok", data: { method: "Fifo", lots: [] } });
    await waitFor(() => expect(document.getElementById("holding-lots-empty")).not.toBeNull());
  });

  // Each lot is a row with a stable id; a lot taken whole is dimmed, free shares named.
  it("lists the lots", async () => {
    mockGetHoldingLots.mockResolvedValue({
      status: "ok",
      data: {
        method: "Fifo",
        lots: [
          { acquired_on: "2024-02-01", quantity: "10", cost: 0, remaining: "0", free: false },
          { acquired_on: "2024-03-15", quantity: "2", cost: 0, remaining: "2", free: true },
        ],
      },
    });
    render(<HoldingLotsModal isOpen onClose={vi.fn()} accountId="account-1" target={target} />);
    await waitFor(() => expect(screen.getByText("lots.free_shares")).toBeInTheDocument());
    expect(document.querySelectorAll("[id^=holding-lots-row-]")).toHaveLength(2);
  });
});
