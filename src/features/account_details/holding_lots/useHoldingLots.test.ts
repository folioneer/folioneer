import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useHoldingLots } from "./useHoldingLots";

const mockGetHoldingLots = vi.fn();

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    getHoldingLots: (...args: unknown[]) => mockGetHoldingLots(...args),
  },
}));

describe("useHoldingLots (#515, the validated mock; CBM-034)", () => {
  // The holding's lots are read for the account and asset, and shown as rows.
  it("reads the holding's lots", async () => {
    mockGetHoldingLots.mockResolvedValue({
      status: "ok",
      data: {
        method: "Fifo",
        lots: [{ acquired_on: "2024-02-01", quantity: "10", cost: 0, remaining: "4", free: false }],
      },
    });
    const { result } = renderHook(() => useHoldingLots("account-1", "asset-1"));
    await waitFor(() => expect(result.current.rows).toHaveLength(1));
    expect(mockGetHoldingLots).toHaveBeenCalledWith("account-1", "asset-1");
    expect(result.current.error).toBeNull();
  });

  // A refused read shows its reason, never a stale table.
  it("shows the reason a read was refused", async () => {
    mockGetHoldingLots.mockResolvedValue({
      status: "error",
      error: { code: "AccountNotFound", account_id: "x" },
    });
    const { result } = renderHook(() => useHoldingLots("x", "asset-1"));
    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.rows).toBeNull();
  });

  // A call that throws ends in the unknown error.
  it("ends in the unknown error when the call throws", async () => {
    mockGetHoldingLots.mockRejectedValue(new Error("bridge gone"));
    const { result } = renderHook(() => useHoldingLots("account-1", "asset-1"));
    await waitFor(() => expect(result.current.error).toEqual({ key: "error.Unknown" }));
  });
});
