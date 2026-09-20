import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AssetPrice } from "@/bindings";

// ── Gateway mock ──────────────────────────────────────────────────────────────
const { mockUpdateAssetPrice } = vi.hoisted(() => ({
  mockUpdateAssetPrice: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    updateAssetPrice: (...args: unknown[]) => mockUpdateAssetPrice(...args),
  },
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn() },
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────
// 100.5 → 100_500_000 micros
const TARGET: AssetPrice = {
  asset_id: "asset-1",
  date: "2026-04-01",
  price: 100_500_000,
  source: "Manual",
};

// ── Hook import (does not exist yet — tests must fail) ────────────────────────
import { useEditPrice } from "./useEditPrice";

describe("useEditPrice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // MKT-081 — The form pre-fills date from target and price as a decimal string
  // (micros / 1_000_000, e.g. 100_500_000 → "100.5").
  it("pre-fills date and price from target on mount", () => {
    const onSuccess = vi.fn();
    const { result } = renderHook(() =>
      useEditPrice({ assetId: "asset-1", target: TARGET, onSuccess }),
    );

    expect(result.current.date).toBe("2026-04-01");
    // Decimal representation — trailing zeros beyond significance may vary;
    // the key constraint is that parseFloat produces the correct value.
    expect(parseFloat(result.current.price)).toBeCloseTo(100.5);
  });

  // MKT-082 — isValid is false when price is empty or non-positive.
  it("isValid is false when price is empty", () => {
    const onSuccess = vi.fn();
    const { result } = renderHook(() =>
      useEditPrice({ assetId: "asset-1", target: TARGET, onSuccess }),
    );

    act(() => {
      result.current.setPrice("");
    });

    expect(result.current.isValid).toBe(false);
  });

  it("isValid is false when price is zero", () => {
    const onSuccess = vi.fn();
    const { result } = renderHook(() =>
      useEditPrice({ assetId: "asset-1", target: TARGET, onSuccess }),
    );

    act(() => {
      result.current.setPrice("0");
    });

    expect(result.current.isValid).toBe(false);
  });

  it("isValid is false when price is negative", () => {
    const onSuccess = vi.fn();
    const { result } = renderHook(() =>
      useEditPrice({ assetId: "asset-1", target: TARGET, onSuccess }),
    );

    act(() => {
      result.current.setPrice("-5");
    });

    expect(result.current.isValid).toBe(false);
  });

  // MKT-082 — isValid is false when date is empty or in the future.
  it("isValid is false when date is empty", () => {
    const onSuccess = vi.fn();
    const { result } = renderHook(() =>
      useEditPrice({ assetId: "asset-1", target: TARGET, onSuccess }),
    );

    act(() => {
      result.current.setDate("");
    });

    expect(result.current.isValid).toBe(false);
  });

  it("isValid is false when date is in the future", () => {
    const onSuccess = vi.fn();
    const { result } = renderHook(() =>
      useEditPrice({ assetId: "asset-1", target: TARGET, onSuccess }),
    );

    act(() => {
      result.current.setDate("2099-12-31");
      result.current.setPrice("100");
    });

    expect(result.current.isValid).toBe(false);
  });

  // Gateway argument verification — newPrice must be the parsed float, NOT micros.
  // The gateway receives the human-readable decimal; the backend converts to micros.
  it("calls updateAssetPrice with assetId, originalDate, newDate and newPrice as number", async () => {
    mockUpdateAssetPrice.mockResolvedValue({ status: "ok", data: null });
    const onSuccess = vi.fn();

    const { result } = renderHook(() =>
      useEditPrice({ assetId: "asset-1", target: TARGET, onSuccess }),
    );

    act(() => {
      result.current.setDate("2026-04-10");
      result.current.setPrice("150.75");
    });

    await act(async () => {
      await result.current.handleSubmit();
    });

    expect(mockUpdateAssetPrice).toHaveBeenCalledWith(
      "asset-1", // assetId
      "2026-04-01", // originalDate — the target's date, not the edited one
      "2026-04-10", // newDate
      150.75, // newPrice — float, not micros
    );
  });

  // MKT-094 — isSubmitting is true while the gateway call is in-flight and false after.
  it("sets isSubmitting true while in flight and false after", async () => {
    let resolveUpdate!: () => void;
    mockUpdateAssetPrice.mockReturnValue(
      new Promise<{ status: string; data: null }>((resolve) => {
        resolveUpdate = () => resolve({ status: "ok", data: null });
      }),
    );

    const onSuccess = vi.fn();
    const { result } = renderHook(() =>
      useEditPrice({ assetId: "asset-1", target: TARGET, onSuccess }),
    );

    // Form is pre-filled from target — isValid should be true as-is.
    expect(result.current.isValid).toBe(true);

    let submitPromise: Promise<void>;
    act(() => {
      submitPromise = result.current.handleSubmit();
    });

    expect(result.current.isSubmitting).toBe(true);

    await act(async () => {
      resolveUpdate();
      await submitPromise;
    });

    expect(result.current.isSubmitting).toBe(false);
  });

  // MKT-086 — onSuccess is called and error is cleared on a successful submit.
  it("calls onSuccess and clears error on successful submit", async () => {
    // Prime with a prior error so we can verify it gets cleared.
    mockUpdateAssetPrice
      .mockResolvedValueOnce({
        status: "error",
        error: { code: "PriceNotFound", asset_id: "asset-1", date: "2026-04-01" },
      })
      .mockResolvedValueOnce({ status: "ok", data: null });

    const onSuccess = vi.fn();
    const { result } = renderHook(() =>
      useEditPrice({ assetId: "asset-1", target: TARGET, onSuccess }),
    );

    // First call → error
    await act(async () => {
      await result.current.handleSubmit();
    });
    expect(result.current.error).toEqual({ key: "error.PriceNotFound" });
    expect(onSuccess).not.toHaveBeenCalled();

    // Second call → success
    await act(async () => {
      await result.current.handleSubmit();
    });
    expect(onSuccess).toHaveBeenCalledOnce();
    expect(result.current.error).toBeNull();
  });

  // MKT-087 — On failure, the form stays open (no onSuccess call) and error is set
  // to the error code returned by the gateway.
  it("keeps form open and sets error code on failure", async () => {
    mockUpdateAssetPrice.mockResolvedValue({
      status: "error",
      error: { code: "DateInFuture" },
    });

    const onSuccess = vi.fn();
    const { result } = renderHook(() =>
      useEditPrice({ assetId: "asset-1", target: TARGET, onSuccess }),
    );

    await act(async () => {
      await result.current.handleSubmit();
    });

    expect(onSuccess).not.toHaveBeenCalled();
    expect(result.current.error).toEqual({ key: "error.DateInFuture" });
    expect(result.current.isSubmitting).toBe(false);
  });
});
