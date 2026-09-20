import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Event } from "@/bindings";
import { useAppStore } from "@/lib/store";

// Capture the gateway's event callback so the tests can fire events.
let capturedEventListener: ((type: Event["type"]) => void) | null = null;

vi.mock("../gateway", () => ({
  getPriceFreshness: vi.fn(),
  subscribeToEvents: vi.fn((cb: (type: Event["type"]) => void) => {
    capturedEventListener = cb;
    return Promise.resolve(() => {});
  }),
}));

import * as gateway from "../gateway";
import { usePriceFreshness } from "./usePriceFreshness";

const fire = (type: Event["type"]) => act(() => capturedEventListener?.(type));

describe("usePriceFreshness (MKT-202/203)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedEventListener = null;
    useAppStore.setState({ priceFetch: { active: false, done: 0, total: 0 } });
    vi.mocked(gateway.getPriceFreshness).mockResolvedValue({
      status: "ok",
      data: { newest_price_date: "2026-09-15", last_fetch_at: "2026-08-28T09:30:28" },
    });
  });

  it("MKT-202: reads the two figures on mount", async () => {
    const { result } = renderHook(() => usePriceFreshness());

    expect(result.current.visible).toBe(false);
    await waitFor(() => expect(result.current.newestPriceDate).toBe("2026-09-15"));
    expect(result.current.visible).toBe(true);
    expect(result.current.lastFetchAt).toBe("2026-08-28T09:30:28");
  });

  it("MKT-202: stays hidden when the read fails", async () => {
    vi.mocked(gateway.getPriceFreshness).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    const { result } = renderHook(() => usePriceFreshness());

    await waitFor(() => expect(gateway.getPriceFreshness).toHaveBeenCalledTimes(1));
    expect(result.current.visible).toBe(false);
  });

  it.each([
    "AssetPriceFetchCompleted",
    "AssetPriceUpdated",
    "TransactionUpdated",
    "SyncCompleted",
  ] as const)("MKT-203: reads again on %s", async (type) => {
    renderHook(() => usePriceFreshness());
    await waitFor(() => expect(gateway.getPriceFreshness).toHaveBeenCalledTimes(1));

    await fire(type);

    await waitFor(() => expect(gateway.getPriceFreshness).toHaveBeenCalledTimes(2));
  });

  it("MKT-203: ignores AssetPriceUpdated while a fetch task runs", async () => {
    renderHook(() => usePriceFreshness());
    await waitFor(() => expect(gateway.getPriceFreshness).toHaveBeenCalledTimes(1));
    useAppStore.setState({ priceFetch: { active: true, done: 1, total: 8 } });

    await fire("AssetPriceUpdated");
    expect(gateway.getPriceFreshness).toHaveBeenCalledTimes(1);

    await fire("AssetPriceFetchCompleted");
    await waitFor(() => expect(gateway.getPriceFreshness).toHaveBeenCalledTimes(2));
  });

  it("MKT-203: ignores events that change neither prices nor what is held", async () => {
    renderHook(() => usePriceFreshness());
    await waitFor(() => expect(gateway.getPriceFreshness).toHaveBeenCalledTimes(1));

    await fire("CategoryUpdated");

    expect(gateway.getPriceFreshness).toHaveBeenCalledTimes(1);
  });
});
