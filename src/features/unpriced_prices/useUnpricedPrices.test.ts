import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UnpricedAsset } from "@/bindings";

// Mock the feature gateway before importing the hook under test.
// The hook calls unpricedPricesGateway.recordPrice — mock at this boundary (F3).
vi.mock("./gateway", () => ({
  unpricedPricesGateway: {
    recordPrice: vi.fn(),
  },
}));

import * as gateway from "./gateway";
import { useUnpricedPrices } from "./useUnpricedPrices";

// Stable test fixtures — defined outside renderHook callback to avoid
// infinite-loop from new reference on every render (per test_convention.md).
const makeAsset = (overrides: Partial<UnpricedAsset> = {}): UnpricedAsset => ({
  asset_id: "asset-1",
  name: "Air Liquide",
  reference: "AI.PA",
  isin: "FR0000120073",
  currency: "EUR",
  last_price: 160_000_000, // 160.00 EUR in micros
  last_price_date: "2026-06-18",
  ...overrides,
});

const TWO_ASSETS: UnpricedAsset[] = [
  makeAsset({ asset_id: "asset-1", name: "Air Liquide", reference: "AI.PA" }),
  makeAsset({
    asset_id: "asset-2",
    name: "LVMH",
    reference: "MC.PA",
    last_price: null,
    last_price_date: null,
  }),
];

describe("useUnpricedPrices — initial state", () => {
  beforeEach(() => vi.clearAllMocks());

  it("exposes one row per UnpricedAsset", () => {
    const { result } = renderHook(() => useUnpricedPrices(TWO_ASSETS, vi.fn()));
    expect(result.current.rows).toHaveLength(2);
  });

  it("each row carries asset_id, name, reference, isin, currency, last_price, last_price_date", () => {
    const asset = makeAsset();
    const { result } = renderHook(() => useUnpricedPrices([asset], vi.fn()));
    const row = result.current.rows[0];
    expect(row).toBeDefined();
    if (row) {
      expect(row.asset_id).toBe("asset-1");
      expect(row.name).toBe("Air Liquide");
      expect(row.reference).toBe("AI.PA");
      expect(row.isin).toBe("FR0000120073");
      expect(row.currency).toBe("EUR");
      expect(row.last_price).toBe(160_000_000);
      expect(row.last_price_date).toBe("2026-06-18");
    }
  });

  it("each row starts with isSubmitting false and no error", () => {
    const { result } = renderHook(() => useUnpricedPrices(TWO_ASSETS, vi.fn()));
    for (const row of result.current.rows) {
      expect(row.isSubmitting).toBe(false);
      expect(row.error).toBeNull();
    }
  });
});

describe("useUnpricedPrices — record (MKT-175)", () => {
  beforeEach(() => vi.clearAllMocks());

  // MKT-175 — confirm calls gateway with today's local ISO date.
  it("record calls gateway.recordPrice with asset_id, today's ISO date, and the entered price", async () => {
    vi.mocked(gateway.unpricedPricesGateway.recordPrice).mockResolvedValue({
      status: "ok",
      data: null,
    });
    const TODAY = new Date().toISOString().slice(0, 10);
    const asset = makeAsset();
    const { result } = renderHook(() => useUnpricedPrices([asset], vi.fn()));

    await act(async () => {
      await result.current.record("asset-1", 15.5);
    });

    expect(gateway.unpricedPricesGateway.recordPrice).toHaveBeenCalledWith("asset-1", TODAY, 15.5);
  });

  // MKT-177 — on success, the row is removed from the list.
  it("record removes the row on success (MKT-177)", async () => {
    vi.mocked(gateway.unpricedPricesGateway.recordPrice).mockResolvedValue({
      status: "ok",
      data: null,
    });
    const { result } = renderHook(() => useUnpricedPrices(TWO_ASSETS, vi.fn()));

    await act(async () => {
      await result.current.record("asset-1", 100);
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0]?.asset_id).toBe("asset-2");
  });

  // MKT-178 — in-flight: isSubmitting is true while gateway call is pending.
  it("record sets isSubmitting true on the row while in flight (MKT-178)", async () => {
    let resolveCall!: (v: { status: "ok"; data: null }) => void;
    vi.mocked(gateway.unpricedPricesGateway.recordPrice).mockReturnValue(
      new Promise((resolve) => {
        resolveCall = resolve;
      }),
    );
    const asset = makeAsset();
    const { result } = renderHook(() => useUnpricedPrices([asset], vi.fn()));

    act(() => {
      void result.current.record("asset-1", 100);
    });

    await waitFor(() => {
      const row = result.current.rows.find((r) => r.asset_id === "asset-1");
      expect(row?.isSubmitting).toBe(true);
    });

    await act(async () => {
      resolveCall({ status: "ok", data: null });
    });

    // Row is removed after success, so isSubmitting no longer visible.
    expect(result.current.rows.find((r) => r.asset_id === "asset-1")).toBeUndefined();
  });

  // MKT-178 — on gateway error, the row stays and exposes an inline error.
  it("record keeps the row and sets per-row error on gateway failure (MKT-178)", async () => {
    vi.mocked(gateway.unpricedPricesGateway.recordPrice).mockResolvedValue({
      status: "error",
      error: { code: "NotPositive" },
    });
    const asset = makeAsset();
    const { result } = renderHook(() => useUnpricedPrices([asset], vi.fn()));

    await act(async () => {
      await result.current.record("asset-1", 0);
    });

    const row = result.current.rows.find((r) => r.asset_id === "asset-1");
    expect(row).toBeDefined();
    expect(row?.error).toEqual({ key: "error.NotPositive" });
    expect(row?.isSubmitting).toBe(false);
  });

  it("record clears isSubmitting on gateway failure (MKT-178)", async () => {
    vi.mocked(gateway.unpricedPricesGateway.recordPrice).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const asset = makeAsset();
    const { result } = renderHook(() => useUnpricedPrices([asset], vi.fn()));

    await act(async () => {
      await result.current.record("asset-1", 10);
    });

    const row = result.current.rows.find((r) => r.asset_id === "asset-1");
    expect(row?.isSubmitting).toBe(false);
  });
});

describe("useUnpricedPrices — skip (MKT-176)", () => {
  beforeEach(() => vi.clearAllMocks());

  // MKT-176 — skip removes the row without calling the gateway.
  it("skip removes the row without calling the gateway (MKT-176)", () => {
    const { result } = renderHook(() => useUnpricedPrices(TWO_ASSETS, vi.fn()));

    act(() => {
      result.current.skip("asset-1");
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0]?.asset_id).toBe("asset-2");
    expect(gateway.unpricedPricesGateway.recordPrice).not.toHaveBeenCalled();
  });
});

describe("useUnpricedPrices — all resolved / signal close (MKT-177)", () => {
  beforeEach(() => vi.clearAllMocks());

  // MKT-177 — when the last row is removed (via record success), onClose is called.
  it("calls onClose when the last row is resolved via record success (MKT-177)", async () => {
    vi.mocked(gateway.unpricedPricesGateway.recordPrice).mockResolvedValue({
      status: "ok",
      data: null,
    });
    const onClose = vi.fn();
    const singleAsset = [makeAsset()];
    const { result } = renderHook(() => useUnpricedPrices(singleAsset, onClose));

    await act(async () => {
      await result.current.record("asset-1", 100);
    });

    expect(onClose).toHaveBeenCalledOnce();
  });

  // MKT-177 — when the last row is removed via skip, onClose is called.
  it("calls onClose when the last row is skipped (MKT-177)", () => {
    const onClose = vi.fn();
    const singleAsset = [makeAsset()];
    const { result } = renderHook(() => useUnpricedPrices(singleAsset, onClose));

    act(() => {
      result.current.skip("asset-1");
    });

    expect(onClose).toHaveBeenCalledOnce();
  });

  // MKT-177 — onClose is NOT called while rows remain.
  it("does not call onClose while rows remain", async () => {
    vi.mocked(gateway.unpricedPricesGateway.recordPrice).mockResolvedValue({
      status: "ok",
      data: null,
    });
    const onClose = vi.fn();
    const { result } = renderHook(() => useUnpricedPrices(TWO_ASSETS, onClose));

    await act(async () => {
      await result.current.record("asset-1", 100);
    });

    // Still one row remaining — onClose must not fire yet.
    expect(onClose).not.toHaveBeenCalled();
    expect(result.current.rows).toHaveLength(1);
  });
});
