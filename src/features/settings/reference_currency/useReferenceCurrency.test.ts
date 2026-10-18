import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReferenceCurrency } from "./useReferenceCurrency";

const mockGet = vi.fn();
const mockSet = vi.fn();
const mockBackfill = vi.fn();
vi.mock("../gateway", () => ({
  getReferenceCurrency: (...args: unknown[]) => mockGet(...args),
  setReferenceCurrency: (...args: unknown[]) => mockSet(...args),
  backfillCurrencyRateHistory: (...args: unknown[]) => mockBackfill(...args),
}));

describe("useReferenceCurrency (FXR-127, FXR-130, FXR-131)", () => {
  beforeEach(() => {
    mockGet.mockReset().mockResolvedValue({
      status: "ok",
      data: { currency: "EUR", choices: ["CHF", "EUR", "USD"] },
    });
    mockSet.mockReset();
    mockBackfill.mockReset();
  });

  it("reads the reference currency and its choices", async () => {
    const { result } = renderHook(() => useReferenceCurrency());
    await waitFor(() => expect(result.current.currency).toBe("EUR"));
    expect(result.current.choices).toEqual(["CHF", "EUR", "USD"]);
  });

  // FXR-130 — a choice is stored, then the rate histories are fetched while it says so.
  it("stores a choice, then fetches the rate histories", async () => {
    mockSet.mockResolvedValue({ status: "ok", data: null });
    let finishFetch: (value: unknown) => void = () => {};
    mockBackfill.mockReturnValue(new Promise((resolve) => (finishFetch = resolve)));
    const { result } = renderHook(() => useReferenceCurrency());
    await waitFor(() => expect(result.current.currency).toBe("EUR"));

    let choosing: Promise<void> = Promise.resolve();
    act(() => {
      choosing = result.current.choose("CHF");
    });
    await waitFor(() => expect(result.current.isFetchingHistory).toBe(true));
    expect(mockSet).toHaveBeenCalledWith("CHF");
    expect(result.current.currency).toBe("CHF");

    await act(async () => {
      finishFetch({ status: "ok", data: 12 });
      await choosing;
    });
    expect(result.current.isFetchingHistory).toBe(false);
    expect(result.current.error).toBeNull();
  });

  // FXR-131 — a refused choice reverts and says why; nothing is fetched.
  it("reverts a refused choice and says why", async () => {
    mockSet.mockResolvedValue({
      status: "error",
      error: { code: "InvalidCurrency", currency: "XYZ" },
    });
    const { result } = renderHook(() => useReferenceCurrency());
    await waitFor(() => expect(result.current.currency).toBe("EUR"));

    await act(async () => {
      await result.current.choose("XYZ");
    });
    expect(result.current.currency).toBe("EUR");
    expect(result.current.error).toEqual({ key: "error.currency.InvalidCurrency" });
    expect(mockBackfill).not.toHaveBeenCalled();
  });

  // FXR-131 — a fetch the provider refuses is said; the choice stays.
  it("says when the rate histories cannot be fetched", async () => {
    mockSet.mockResolvedValue({ status: "ok", data: null });
    mockBackfill.mockResolvedValue({ status: "error", error: { code: "ProviderUnreachable" } });
    const { result } = renderHook(() => useReferenceCurrency());
    await waitFor(() => expect(result.current.currency).toBe("EUR"));

    await act(async () => {
      await result.current.choose("CHF");
    });
    expect(result.current.currency).toBe("CHF");
    expect(result.current.error).toEqual({ key: "error.currency.ProviderUnreachable" });
  });
});
