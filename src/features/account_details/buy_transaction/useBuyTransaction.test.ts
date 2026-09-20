import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, Asset } from "@/bindings";
import { setDisplayLocale } from "@/lib/microUnits";
import { useAppStore } from "@/lib/store";
import { useBuyTransaction } from "./useBuyTransaction";

const AUTO_RECORD_PRICE_KEY = "auto_record_price";

const { mockBuyHolding, mockRecordAssetPrice, mockGetSnapshot } = vi.hoisted(() => ({
  mockBuyHolding: vi.fn(),
  mockRecordAssetPrice: vi.fn(),
  mockGetSnapshot: vi.fn(),
}));

vi.mock("@/features/transactions/useTransactions", () => ({
  useTransactions: () => ({
    buyHolding: mockBuyHolding,
    sellHolding: vi.fn(),
    correctTransaction: vi.fn(),
    cancelTransaction: vi.fn(),
    getTransactions: vi.fn(),
  }),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    recordAssetPrice: mockRecordAssetPrice,
    getHoldingSnapshotAsOf: mockGetSnapshot,
  },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en" },
  }),
}));

const BASE_PROPS = {
  accountId: "account-1",
  assetId: "asset-1",
};

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

describe("useBuyTransaction", () => {
  beforeEach(() => {
    setDisplayLocale("en");
    localStorage.clear();
    mockBuyHolding.mockReset();
    mockRecordAssetPrice.mockReset();
    mockGetSnapshot.mockReset();
    mockGetSnapshot.mockResolvedValue({ status: "ok", data: { quantity: 0, average_price: 0 } });
    useAppStore.setState({
      assets: [{ id: "asset-1", name: "Apple", is_archived: false, currency: "USD" }] as Asset[],
      accounts: [{ id: "account-1", name: "My Account" }] as Account[],
    });
  });

  // MKT-052 — recordPrice defaults to the global toggle value at hook mount (create mode)
  it("recordPrice defaults to false when localStorage auto_record_price is absent", () => {
    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));
    expect(result.current.recordPrice).toBe(false);
  });

  // TDI-020 — average cost is exposed when the asset is held as of the date.
  it("exposes averageCostAsOfDate when the asset is held as of the date", async () => {
    mockGetSnapshot.mockResolvedValue({
      status: "ok",
      data: { quantity: 2_000_000, average_price: 100_000_000 },
    });
    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));
    await waitFor(() => expect(result.current.averageCostAsOfDate).not.toBeNull());
  });

  // TDI-021 — average cost is hidden when nothing is held as of the date.
  it("hides averageCostAsOfDate when nothing is held as of the date", async () => {
    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));
    await waitFor(() => expect(mockGetSnapshot).toHaveBeenCalled());
    expect(result.current.averageCostAsOfDate).toBeNull();
  });

  // MKT-052 — recordPrice is true at mount when localStorage key is "true"
  it("recordPrice defaults to true when localStorage auto_record_price is true", () => {
    localStorage.setItem(AUTO_RECORD_PRICE_KEY, "true");
    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));
    expect(result.current.recordPrice).toBe(true);
  });

  // MKT-053 — snapshot at mount: changing localStorage after mount does not change recordPrice
  it("does not change recordPrice when localStorage is mutated after mount (snapshot semantics)", async () => {
    localStorage.removeItem(AUTO_RECORD_PRICE_KEY);
    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));

    expect(result.current.recordPrice).toBe(false);

    // Mutate localStorage directly without re-rendering
    localStorage.setItem(AUTO_RECORD_PRICE_KEY, "true");

    // recordPrice must remain the snapshot value from mount
    expect(result.current.recordPrice).toBe(false);
  });

  // MKT-054 — calls recordAssetPrice when recordPrice is true and price is non-zero
  it("calls recordAssetPrice when recordPrice is true and price is non-zero", async () => {
    localStorage.setItem(AUTO_RECORD_PRICE_KEY, "true");
    mockBuyHolding.mockResolvedValue({ data: { id: "tx-1" }, error: null });
    mockRecordAssetPrice.mockResolvedValue({ status: "ok", data: null });

    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "100");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "0");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordAssetPrice).toHaveBeenCalledWith("asset-1", "2024-06-01", 100);
  });

  // TRX-060 — the mode defaults to unit-price entry and sends total_amount: null
  it("sends total_amount: null in the default price mode", async () => {
    mockBuyHolding.mockResolvedValue({ data: { id: "tx-3" }, error: null });

    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));
    expect(result.current.entryMode).toBe("price");

    await act(async () => {
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "2");
      result.current.handleChange("unitPrice", "100");
      result.current.handleChange("fees", "10");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockBuyHolding).toHaveBeenCalledWith(
      expect.objectContaining({
        total_amount: null,
        unit_price: 100_000_000,
        quantity: 2_000_000,
        fees: 10_000_000,
      }),
    );
  });

  // TRX-060 — total mode sends the typed total and the FE-derived unit price
  it("sends the typed total_amount and the derived unit_price in total mode", async () => {
    mockBuyHolding.mockResolvedValue({ data: { id: "tx-4" }, error: null });

    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));

    await act(async () => {
      result.current.setEntryMode("total");
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "2");
      result.current.handleChange("fees", "10");
      result.current.handleTotalAmountChange("210");
    });

    // Derived preview: (210 − 10) / 2 = 100 in asset currency
    expect(result.current.unitPriceDisplay).toBe("100.000");

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockBuyHolding).toHaveBeenCalledWith(
      expect.objectContaining({
        total_amount: 210_000_000,
        unit_price: 100_000_000,
        quantity: 2_000_000,
        fees: 10_000_000,
      }),
    );
  });

  // TRX-060 — the derived price display falls back to "—" while quantity is 0
  it("shows an em dash for the derived unit price while quantity is 0", async () => {
    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));

    await act(async () => {
      result.current.setEntryMode("total");
      result.current.handleTotalAmountChange("210");
    });

    expect(result.current.unitPriceDisplay).toBe("—");
  });

  // TRX-060 — a total below the fees blocks submission in total mode
  it("rejects a total below the fees in total mode", async () => {
    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));

    await act(async () => {
      result.current.setEntryMode("total");
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("fees", "10");
      result.current.handleTotalAmountChange("5");
    });

    expect(result.current.isFormValid).toBe(false);

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockBuyHolding).not.toHaveBeenCalled();
    expect(result.current.error).toEqual({
      key: "transaction.error_validation_total_below_fees",
    });
  });

  // TRX-060 — switching price → total seeds the total input from the computed total
  it("seeds the total input from the computed total when switching to total mode", async () => {
    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("quantity", "2");
      result.current.handleChange("unitPrice", "100");
      result.current.handleChange("fees", "10");
    });

    await act(async () => {
      result.current.setEntryMode("total");
    });

    // 2 × 100 + 10 fees = 210
    expect(result.current.entryMode).toBe("total");
    expect(result.current.totalAmountInput).toBe("210.000");
  });

  // TRX-060 — switching total → price seeds the unit-price field from the derived price
  it("seeds the unit-price field from the derived price when switching back to price mode", async () => {
    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));

    await act(async () => {
      result.current.setEntryMode("total");
    });
    await act(async () => {
      result.current.handleChange("quantity", "2");
      result.current.handleChange("fees", "10");
      result.current.handleTotalAmountChange("210");
    });

    await act(async () => {
      result.current.setEntryMode("price");
    });

    // (210 − 10) / 2 = 100
    expect(result.current.entryMode).toBe("price");
    expect(result.current.formData.unitPrice).toBe("100.000");
  });

  // TRX-060 — no carry-over when the current values give nothing to carry
  it("keeps the target field untouched when switching modes without derivable values", async () => {
    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));

    await act(async () => {
      result.current.setEntryMode("total");
    });
    expect(result.current.totalAmountInput).toBe("");

    await act(async () => {
      result.current.setEntryMode("price");
    });
    expect(result.current.formData.unitPrice).toBe("");
  });

  // TRX-060 — the below-fees rejection surfaces as an inline error on the Total field
  it("exposes totalBelowFeesError when the typed total is below the fees in total mode", async () => {
    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));

    await act(async () => {
      result.current.setEntryMode("total");
    });
    await act(async () => {
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("fees", "10");
      result.current.handleTotalAmountChange("5");
    });

    expect(result.current.totalBelowFeesError).toEqual({
      key: "transaction.error_validation_total_below_fees",
    });
    expect(result.current.isFormValid).toBe(false);

    // Raising the total above the fees clears the inline error
    await act(async () => {
      result.current.handleTotalAmountChange("15");
    });
    expect(result.current.totalBelowFeesError).toBeNull();
  });

  // MKT-054 — does not call recordAssetPrice when recordPrice is false
  it("does not call recordAssetPrice when recordPrice is false", async () => {
    localStorage.removeItem(AUTO_RECORD_PRICE_KEY);
    mockBuyHolding.mockResolvedValue({ data: { id: "tx-2" }, error: null });

    const { result } = renderHook(() => useBuyTransaction(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "100");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "0");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordAssetPrice).not.toHaveBeenCalled();
  });
});
