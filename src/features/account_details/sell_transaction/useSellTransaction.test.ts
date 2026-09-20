import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useSellTransaction } from "./useSellTransaction";

const { mockSellHolding, mockRecordAssetPrice, mockGetSnapshot } = vi.hoisted(() => ({
  mockSellHolding: vi.fn(),
  mockRecordAssetPrice: vi.fn(),
  mockGetSnapshot: vi.fn(),
}));

vi.mock("@/features/transactions/useTransactions", () => ({
  useTransactions: () => ({
    buyHolding: vi.fn(),
    sellHolding: mockSellHolding,
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
    t: (key: string, opts?: { max?: string }) => (opts?.max ? `${key}:${opts.max}` : key),
    i18n: { language: "en" },
  }),
}));

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

const BASE_PROPS = {
  accountId: "account-1",
  assetId: "asset-1",
  holdingQuantityMicro: 3_000_000, // 3 units
};

describe("useSellTransaction", () => {
  beforeEach(() => {
    localStorage.clear();
    mockSellHolding.mockReset();
    mockRecordAssetPrice.mockReset();
    mockGetSnapshot.mockReset();
    mockGetSnapshot.mockResolvedValue({ status: "ok", data: { quantity: 0, average_price: 0 } });
  });

  // TDI-030 — potential P&L = proceeds − VWAP cost basis of the sold quantity.
  it("computes potentialPnl from the as-of snapshot and the typed sell", async () => {
    mockGetSnapshot.mockResolvedValue({
      status: "ok",
      data: { quantity: 2_000_000, average_price: 100_000_000 },
    });
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));
    await waitFor(() => expect(result.current.averageCostAsOfDate).not.toBeNull());
    await act(async () => {
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "150");
    });
    // proceeds 150 − cost basis (100 × 1) = 50 → 50_000_000 micro
    expect(result.current.potentialPnl?.raw).toBe(50_000_000);
  });

  // TDI-031 — no potential P&L until a quantity and price are entered.
  it("hides potentialPnl when quantity or price is missing", async () => {
    mockGetSnapshot.mockResolvedValue({
      status: "ok",
      data: { quantity: 2_000_000, average_price: 100_000_000 },
    });
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));
    await waitFor(() => expect(result.current.averageCostAsOfDate).not.toBeNull());
    expect(result.current.potentialPnl).toBeNull();
  });

  // TDI-030 — cross-currency: average_price (account CCY) and proceeds (account CCY,
  // rate-converted) are the same currency, so the P&L is correct even when rate ≠ 1.
  it("computes potentialPnl correctly for a cross-currency sell", async () => {
    mockGetSnapshot.mockResolvedValue({
      status: "ok",
      data: { quantity: 2_000_000, average_price: 100_000_000 },
    });
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));
    await waitFor(() => expect(result.current.averageCostAsOfDate).not.toBeNull());
    await act(async () => {
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "60");
      result.current.handleChange("exchangeRate", "2");
    });
    // proceeds (1 × 60 × 2) = 120 account CCY; cost basis (avg 100 × 1) = 100; P&L = 20.
    expect(result.current.potentialPnl?.raw).toBe(20_000_000);
  });

  // SEL-023 — sell total = floor(floor(qty × price / MICRO) × rate / MICRO) − fees
  it("computes sell total with fees subtracted (SEL-023)", async () => {
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("quantity", "2");
      result.current.handleChange("unitPrice", "50");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "5");
    });

    // 2 × 50 × 1 − 5 = 95
    expect(result.current.totalAmountDisplay).toBe("95,000");
  });

  // SEL-022 — oversell guard: quantity > holdingQuantityMicro → form invalid
  it("marks form invalid when quantity exceeds holding (SEL-022)", async () => {
    const { result } = renderHook(() =>
      useSellTransaction({ ...BASE_PROPS, holdingQuantityMicro: 1_000_000 }),
    );

    await act(async () => {
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "2"); // 2 > 1 unit held
      result.current.handleChange("unitPrice", "50");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "0");
    });

    expect(result.current.isFormValid).toBe(false);
  });

  // SEL-022 — oversell sets the error key on submit attempt
  it("sets oversell error key on submit when quantity exceeds holding (SEL-022)", async () => {
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() =>
      useSellTransaction({
        ...BASE_PROPS,
        holdingQuantityMicro: 1_000_000,
        onSubmitSuccess,
      }),
    );

    await act(async () => {
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "2");
      result.current.handleChange("unitPrice", "50");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "0");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({
      key: "transaction.error_validation_oversell",
      vars: expect.objectContaining({ max: expect.any(String) }),
    });
    expect(mockSellHolding).not.toHaveBeenCalled();
    expect(onSubmitSuccess).not.toHaveBeenCalled();
  });

  // Submit calls sellHolding (not addTransaction with transaction_type)
  it("submits via sellHolding with correct DTO fields", async () => {
    mockSellHolding.mockResolvedValue({ data: { id: "tx-1" }, error: null });
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));

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

    expect(mockSellHolding).toHaveBeenCalledWith(
      expect.objectContaining({
        account_id: "account-1",
        asset_id: "asset-1",
        quantity: 1_000_000,
        unit_price: 100_000_000,
      }),
    );
  });

  // SEL-050 — the mode defaults to unit-price entry and sends total_amount: null
  it("sends total_amount: null in the default price mode", async () => {
    mockSellHolding.mockResolvedValue({ data: { id: "tx-p" }, error: null });
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));
    expect(result.current.entryMode).toBe("price");

    await act(async () => {
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "100");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockSellHolding).toHaveBeenCalledWith(
      expect.objectContaining({ total_amount: null, unit_price: 100_000_000 }),
    );
  });

  // SEL-050 — total mode sends the typed net proceeds and the derived unit price
  // (fees added back: (140 + 10) / 1 = 150 in asset currency)
  it("sends the typed total_amount and the derived unit_price in total mode", async () => {
    mockSellHolding.mockResolvedValue({ data: { id: "tx-t" }, error: null });
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));

    await act(async () => {
      result.current.setEntryMode("total");
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("fees", "10");
      result.current.handleTotalAmountChange("140");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockSellHolding).toHaveBeenCalledWith(
      expect.objectContaining({
        total_amount: 140_000_000,
        unit_price: 150_000_000,
        quantity: 1_000_000,
        fees: 10_000_000,
      }),
    );
  });

  // SEL-050 — the derived price display falls back to "—" while quantity is 0
  it("shows an em dash for the derived unit price while quantity is 0", async () => {
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));

    await act(async () => {
      result.current.setEntryMode("total");
      result.current.handleTotalAmountChange("140");
    });

    expect(result.current.unitPriceDisplay).toBe("—");
  });

  // SEL-050 — switching price → total seeds the total input from the computed proceeds
  it("seeds the total input from the computed proceeds when switching to total mode", async () => {
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("quantity", "2");
      result.current.handleChange("unitPrice", "50");
      result.current.handleChange("fees", "5");
    });

    await act(async () => {
      result.current.setEntryMode("total");
    });

    // 2 × 50 − 5 fees = 95
    expect(result.current.entryMode).toBe("total");
    expect(result.current.totalAmountInput).toBe("95.000");
  });

  // SEL-050 — switching total → price seeds the unit-price field from the derived price
  it("seeds the unit-price field from the derived price when switching back to price mode", async () => {
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));

    await act(async () => {
      result.current.setEntryMode("total");
    });
    await act(async () => {
      result.current.handleChange("quantity", "2");
      result.current.handleChange("fees", "10");
      result.current.handleTotalAmountChange("140");
    });

    await act(async () => {
      result.current.setEntryMode("price");
    });

    // (140 + 10) / 2 = 75
    expect(result.current.entryMode).toBe("price");
    expect(result.current.formData.unitPrice).toBe("75.000");
  });

  // SEL-022 — the oversell guard still applies in total mode
  it("keeps the oversell guard in total mode", async () => {
    const { result } = renderHook(() =>
      useSellTransaction({ ...BASE_PROPS, holdingQuantityMicro: 1_000_000 }),
    );

    await act(async () => {
      result.current.setEntryMode("total");
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "2"); // 2 > 1 unit held
      result.current.handleTotalAmountChange("140");
    });

    expect(result.current.isFormValid).toBe(false);
  });

  // Backend error keeps modal open, sets error, does not call onSubmitSuccess
  it("sets error on backend failure and does not call onSubmitSuccess", async () => {
    mockSellHolding.mockResolvedValue({ data: null, error: { key: "error.DatabaseError" } });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useSellTransaction({ ...BASE_PROPS, onSubmitSuccess }));

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

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(onSubmitSuccess).not.toHaveBeenCalled();
  });

  // SEL-011 — accountId and assetId are pre-filled from props (read-only)
  it("initialises formData accountId and assetId from props (SEL-011)", () => {
    const { result } = renderHook(() =>
      useSellTransaction({
        ...BASE_PROPS,
        accountId: "acc-42",
        assetId: "ast-99",
      }),
    );
    expect(result.current.formData.accountId).toBe("acc-42");
    expect(result.current.formData.assetId).toBe("ast-99");
  });

  // SEL-029 — default form values: date=today, exchangeRate=1.000000, fees=0
  it("initialises defaults: date=today, exchangeRate=1.000000, fees=0 (SEL-029)", () => {
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));
    const expectedToday = new Date().toISOString().slice(0, 10);
    expect(result.current.formData.date).toBe(expectedToday);
    expect(result.current.formData.exchangeRate).toBe("1.000000");
    expect(result.current.formData.fees).toBe("0");
  });

  // SEL-036 — when exchange rate field is hidden (currencies match), default 1.000000 is submitted
  it("submits exchangeRate=1000000 micro when using default (SEL-036)", async () => {
    mockSellHolding.mockResolvedValue({ data: { id: "tx-3" }, error: null });
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "100");
      // exchangeRate left at default "1.000000"
      result.current.handleChange("fees", "0");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockSellHolding).toHaveBeenCalledWith(
      expect.objectContaining({ exchange_rate: 1_000_000 }),
    );
  });

  // Success calls onSubmitSuccess
  it("calls onSubmitSuccess on successful submit", async () => {
    mockSellHolding.mockResolvedValue({ data: { id: "tx-2" }, error: null });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useSellTransaction({ ...BASE_PROPS, onSubmitSuccess }));

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

    expect(result.current.error).toBeNull();
    expect(onSubmitSuccess).toHaveBeenCalledTimes(1);
  });

  // MKT-052 — recordPrice defaults to global toggle value at hook mount
  it("recordPrice defaults to false when localStorage auto_record_price is absent", () => {
    localStorage.removeItem("auto_record_price");
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));
    expect(result.current.recordPrice).toBe(false);
  });

  // MKT-052 — recordPrice is true at mount when localStorage key is "true"
  it("recordPrice defaults to true when localStorage auto_record_price is true", () => {
    localStorage.setItem("auto_record_price", "true");
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));
    expect(result.current.recordPrice).toBe(true);
  });

  // MKT-053 — snapshot at mount: mutating localStorage after mount does not change recordPrice
  it("does not change recordPrice when localStorage is mutated after mount (snapshot semantics)", () => {
    localStorage.removeItem("auto_record_price");
    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));

    expect(result.current.recordPrice).toBe(false);

    localStorage.setItem("auto_record_price", "true");

    expect(result.current.recordPrice).toBe(false);
  });

  // MKT-054 — calls recordAssetPrice when recordPrice is true and price is non-zero
  it("calls recordAssetPrice when recordPrice is true and price is non-zero", async () => {
    localStorage.setItem("auto_record_price", "true");
    mockSellHolding.mockResolvedValue({
      data: { id: "tx-sell-1" },
      error: null,
    });
    mockRecordAssetPrice.mockResolvedValue({ status: "ok", data: null });

    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));

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

  // MKT-054 — does not call recordAssetPrice when recordPrice is false
  it("does not call recordAssetPrice when recordPrice is false", async () => {
    localStorage.removeItem("auto_record_price");
    mockSellHolding.mockResolvedValue({
      data: { id: "tx-sell-2" },
      error: null,
    });

    const { result } = renderHook(() => useSellTransaction(BASE_PROPS));

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
