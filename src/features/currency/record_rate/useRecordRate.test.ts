import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CurrencyRate } from "@/bindings";
import { setDisplayLocale } from "@/lib/microUnits";
import { useRecordRate } from "./useRecordRate";

const { mockRecord, mockUpdate } = vi.hoisted(() => ({
  mockRecord: vi.fn(),
  mockUpdate: vi.fn(),
}));

vi.mock("../gateway", () => ({
  recordCurrencyRate: mockRecord,
  updateCurrencyRate: mockUpdate,
}));

vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => vi.fn(),
}));

const recorded: CurrencyRate = {
  from_currency: "USD",
  to_currency: "EUR",
  date: "2026-01-02",
  rate: 1_084_500,
  source: "Manual",
} as CurrencyRate;

const props = { fromCurrency: "USD", toCurrency: "EUR", onSuccess: vi.fn() };

describe("useRecordRate — the rate field by language (NUM-010/011)", () => {
  beforeEach(() => {
    mockRecord.mockReset().mockResolvedValue({ status: "ok", data: null });
    mockUpdate.mockReset().mockResolvedValue({ status: "ok", data: null });
  });
  afterEach(() => setDisplayLocale("fr"));

  // In French the field shows a comma, reads a comma or a dot, and records the figure.
  it("in French, reads a comma or a dot and shows a comma", async () => {
    setDisplayLocale("fr");
    const { result } = renderHook(() => useRecordRate(props));
    act(() => result.current.setDate("2026-01-02"));

    act(() => result.current.setRate("1.09"));
    expect(result.current.rate).toBe("1,09");
    act(() => result.current.setRate("1,0845"));
    expect(result.current.rate).toBe("1,0845");

    await act(async () => {
      await result.current.submit();
    });
    expect(mockRecord).toHaveBeenCalledWith("USD", "EUR", "2026-01-02", 1.0845);
  });

  // In English only the dot is read: a comma is no number, and is sent as none.
  it("in English, shows the dot and reads a comma as no number", async () => {
    setDisplayLocale("en");
    const { result } = renderHook(() => useRecordRate(props));
    act(() => result.current.setDate("2026-01-02"));
    act(() => result.current.setRate("1,09"));
    expect(result.current.rate).toBe("1,09");

    await act(async () => {
      await result.current.submit();
    });
    expect(mockRecord).toHaveBeenCalledWith("USD", "EUR", "2026-01-02", Number.NaN);
  });

  // TRX-024 — a rate being corrected fills the field with every decimal recorded, so
  // saving it untouched sends back the same rate.
  it("fills a corrected rate exactly, and sends it back unchanged", async () => {
    setDisplayLocale("fr");
    const { result } = renderHook(() => useRecordRate({ ...props, initialRate: recorded }));
    expect(result.current.rate).toBe("1,0845");

    await act(async () => {
      await result.current.submit();
    });
    expect(mockUpdate).toHaveBeenCalledWith("USD", "EUR", "2026-01-02", "2026-01-02", 1.0845);
  });
});
