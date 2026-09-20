import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AssetPrice } from "@/bindings";

// ── Gateway mock ──────────────────────────────────────────────────────────────
// vi.hoisted ensures the spy references exist before vi.mock is hoisted.
const { mockGetAssetPrices, mockDeleteAssetPrice } = vi.hoisted(() => ({
  mockGetAssetPrices: vi.fn(),
  mockDeleteAssetPrice: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    getAssetPrices: (...args: unknown[]) => mockGetAssetPrices(...args),
    deleteAssetPrice: (...args: unknown[]) => mockDeleteAssetPrice(...args),
  },
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn() },
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────
const makePrice = (overrides: Partial<AssetPrice> = {}): AssetPrice => ({
  asset_id: "asset-1",
  date: "2026-04-01",
  price: 100_500_000, // 100.5 in micros
  source: "Manual",
  ...overrides,
});

// ── Hook import (does not exist yet — tests must fail) ────────────────────────
import { usePriceHistory } from "./usePriceHistory";

describe("usePriceHistory", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // MKT-072 — On mount the hook calls getAssetPrices and exposes the result as prices.
  it("loads prices on mount and exposes them as returned by the gateway", async () => {
    const rows: AssetPrice[] = [
      makePrice({ date: "2026-04-01", price: 100_500_000 }),
      makePrice({ date: "2026-03-15", price: 95_000_000 }),
    ];
    mockGetAssetPrices.mockResolvedValue({ status: "ok", data: rows });

    const { result } = renderHook(() => usePriceHistory({ assetId: "asset-1" }));

    // isLoading true while the call is in-flight
    expect(result.current.isLoading).toBe(true);

    await act(async () => {});

    expect(result.current.isLoading).toBe(false);
    expect(result.current.prices).toEqual(rows);
    expect(result.current.fetchError).toBeNull();
    expect(mockGetAssetPrices).toHaveBeenCalledWith("asset-1");
  });

  // MKT-074 — When the gateway returns an error the hook surfaces it as fetchError
  // and keeps prices empty.
  it("surfaces fetchError when gateway returns an error", async () => {
    mockGetAssetPrices.mockResolvedValue({
      status: "error",
      error: { code: "AssetNotFound", id: "asset-unknown" },
    });

    const { result } = renderHook(() => usePriceHistory({ assetId: "asset-unknown" }));

    await act(async () => {});

    expect(result.current.prices).toEqual([]);
    expect(result.current.fetchError).toEqual({ key: "error.AssetNotFound" });
    expect(result.current.isLoading).toBe(false);
    expect(mockGetAssetPrices).toHaveBeenCalledWith("asset-unknown");
  });

  // MKT-076 — Calling refetch() triggers another gateway call and updates prices.
  it("refetch re-calls the gateway and updates prices", async () => {
    const initial: AssetPrice[] = [makePrice({ price: 100_000_000 })];
    const updated: AssetPrice[] = [
      makePrice({ price: 110_000_000 }),
      makePrice({ date: "2026-03-20", price: 105_000_000 }),
    ];

    mockGetAssetPrices
      .mockResolvedValueOnce({ status: "ok", data: initial })
      .mockResolvedValueOnce({ status: "ok", data: updated });

    const { result } = renderHook(() => usePriceHistory({ assetId: "asset-1" }));
    await act(async () => {});

    expect(result.current.prices).toEqual(initial);
    const callsBefore = mockGetAssetPrices.mock.calls.length;

    await act(async () => {
      result.current.refetch();
    });

    expect(mockGetAssetPrices.mock.calls.length).toBeGreaterThan(callsBefore);
    expect(result.current.prices).toEqual(updated);
  });

  // MKT-093 — deletingDate is set to the target date while confirmDelete is in-flight
  // and is cleared (null) after the delete resolves successfully.
  it("sets deletingDate while delete is in flight and clears it on success", async () => {
    mockGetAssetPrices.mockResolvedValue({
      status: "ok",
      data: [makePrice({ date: "2026-04-01" })],
    });

    let resolveDelete!: () => void;
    mockDeleteAssetPrice.mockReturnValue(
      new Promise<{ status: string; data: null }>((resolve) => {
        resolveDelete = () => resolve({ status: "ok", data: null });
      }),
    );

    const { result } = renderHook(() => usePriceHistory({ assetId: "asset-1" }));
    await act(async () => {});

    // deletingDate starts as null
    expect(result.current.deletingDate).toBeNull();

    let deletePromise: Promise<boolean>;
    act(() => {
      deletePromise = result.current.confirmDelete("2026-04-01");
    });

    // While in-flight, deletingDate equals the date being deleted
    expect(result.current.deletingDate).toBe("2026-04-01");

    await act(async () => {
      resolveDelete();
      await deletePromise;
    });

    // After success, deletingDate is cleared
    expect(result.current.deletingDate).toBeNull();
    expect(mockDeleteAssetPrice).toHaveBeenCalledWith("asset-1", "2026-04-01");
  });

  // MKT-096 — When delete fails, the entry stays in prices and deleteError is set.
  it("keeps the entry in prices and sets deleteError when delete fails", async () => {
    const rows: AssetPrice[] = [makePrice({ date: "2026-04-01" })];
    mockGetAssetPrices.mockResolvedValue({ status: "ok", data: rows });
    mockDeleteAssetPrice.mockResolvedValue({
      status: "error",
      error: { code: "PriceNotFound", asset_id: "asset-1", date: "2026-04-01" },
    });

    const { result } = renderHook(() => usePriceHistory({ assetId: "asset-1" }));
    await act(async () => {});

    await act(async () => {
      await result.current.confirmDelete("2026-04-01");
    });

    // Entry must still be present
    expect(result.current.prices).toEqual(rows);
    expect(result.current.deleteError).toEqual({ key: "error.PriceNotFound" });
    expect(result.current.deletingDate).toBeNull();
  });

  // deleteError is cleared when a subsequent delete succeeds.
  it("clears deleteError when a subsequent delete succeeds", async () => {
    const rows: AssetPrice[] = [makePrice({ date: "2026-04-01" })];
    mockGetAssetPrices.mockResolvedValue({ status: "ok", data: rows });

    // First call fails, second succeeds
    mockDeleteAssetPrice
      .mockResolvedValueOnce({ status: "error", error: { code: "DatabaseError" } })
      .mockResolvedValueOnce({ status: "ok", data: null });

    // Second getAssetPrices call (after successful delete) returns empty
    mockGetAssetPrices.mockResolvedValueOnce({ status: "ok", data: [] });

    const { result } = renderHook(() => usePriceHistory({ assetId: "asset-1" }));
    await act(async () => {});

    // Trigger a failing delete
    await act(async () => {
      await result.current.confirmDelete("2026-04-01");
    });
    expect(result.current.deleteError).toEqual({ key: "error.DatabaseError" });

    // Trigger a succeeding delete — deleteError should be cleared
    await act(async () => {
      await result.current.confirmDelete("2026-04-01");
    });
    expect(result.current.deleteError).toBeNull();
  });
});
