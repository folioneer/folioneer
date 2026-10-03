import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, Asset, Transaction, TransactionDraft } from "@/bindings";
import { microToFormatted } from "@/lib/microUnits";
import { useAppStore } from "@/lib/store";
import { useEditTransactionModal } from "./useEditTransactionModal";

const { mockCorrectTransaction, mockRecordAssetPrice, mockValidateDraft } = vi.hoisted(() => ({
  mockCorrectTransaction: vi.fn(),
  mockRecordAssetPrice: vi.fn(),
  mockValidateDraft: vi.fn(),
}));

vi.mock("../useTransactions", () => ({
  useTransactions: () => ({
    buyHolding: vi.fn(),
    sellHolding: vi.fn(),
    correctTransaction: mockCorrectTransaction,
    cancelTransaction: vi.fn(),
    getTransactions: vi.fn(),
  }),
}));

vi.mock("../gateway", () => ({
  transactionGateway: {
    recordAssetPrice: mockRecordAssetPrice,
    validateTransactionDraft: mockValidateDraft,
  },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "fr" },
  }),
}));

const MICRO = 1_000_000;

// The draft check (TRX-062) is the backend's: this stand-in reports a missing account, asset
// or date, echoes a typed unit price, and otherwise returns fixed figures.
const fakeDraftCheck = async (draft: TransactionDraft) => {
  if (!draft.account_id) return { status: "error", error: { code: "AccountMissing" } };
  if (!draft.asset_id) return { status: "error", error: { code: "AssetMissing" } };
  if (!draft.date) return { status: "error", error: { code: "DateMissing" } };
  const unit_price = draft.entered.mode === "UnitPrice" ? draft.entered.unit_price : 7_000_000;
  return { status: "ok", data: { unit_price, total_amount: 42_000_000 } };
};

// The check's answer by entry mode: a typed total, or the unit price.
const answersByMode =
  (byPrice: [number, number], byTotal: [number, number]) => async (draft: TransactionDraft) => {
    const [unit_price, total_amount] = draft.entered.mode === "Total" ? byTotal : byPrice;
    return { status: "ok", data: { unit_price, total_amount } };
  };

// 2 units @ 50.0 each, rate=1.0, fees=0 → total=100_000_000
const baseTransaction: Transaction = {
  id: "tx-existing",
  account_id: "account-1",
  asset_id: "asset-1",
  transaction_type: "Purchase",
  date: "2024-01-10",
  quantity: 2 * MICRO,
  unit_price: 50 * MICRO,
  exchange_rate: 1 * MICRO,
  fees: 0,
  total_amount: 100 * MICRO,
  note: "initial note",
  realized_pnl: null,
  created_at: "2024-01-10T00:00:00Z",
};

// unit_price=0, fees=5 → totalMicro=5*MICRO > 0, passes validation; used for MKT-061
const zeroUnitPriceTransaction: Transaction = {
  ...baseTransaction,
  id: "tx-zero-price",
  unit_price: 0,
  fees: 5 * MICRO,
  total_amount: 5 * MICRO,
};

// TRX-051: 2 units @ total_cost=100 → stored unit_price=50 (computed by backend, TRX-047)
const openingBalanceTransaction: Transaction = {
  ...baseTransaction,
  id: "tx-ob",
  transaction_type: "OpeningBalance",
  unit_price: 50 * MICRO,
  total_amount: 100 * MICRO,
  fees: 0,
  note: null,
};

describe("useEditTransactionModal", () => {
  beforeEach(() => {
    localStorage.clear();
    mockCorrectTransaction.mockReset();
    mockRecordAssetPrice.mockReset();
    mockValidateDraft.mockReset().mockImplementation(fakeDraftCheck);
    useAppStore.setState({
      assets: [
        { id: "asset-1", name: "Apple", is_archived: false, currency: "USD" },
        { id: "asset-archived", name: "OldCo", is_archived: true, currency: "USD" },
      ] as Asset[],
      accounts: [{ id: "account-1", name: "My Account" }] as Account[],
    });
  });

  // Pre-fill: micro-unit values are converted to decimal strings; the total comes from the check
  it("pre-fills formData from transaction (micro → decimal)", async () => {
    mockValidateDraft.mockImplementation(answersByMode([50 * MICRO, 100 * MICRO], [0, 0]));
    const { result } = renderHook(() => useEditTransactionModal({ transaction: baseTransaction }));
    expect(result.current.formData.quantity).toBe("2.000");
    expect(result.current.formData.unitPrice).toBe("50.000");
    expect(result.current.formData.exchangeRate).toBe("1.000");
    expect(result.current.formData.note).toBe("initial note");
    await waitFor(() => expect(result.current.totalAmountDisplay).toBe("100,000"));
    // TRX-063 — a correction names the transaction it replaces
    expect(mockValidateDraft).toHaveBeenLastCalledWith(
      expect.objectContaining({ kind: "Purchase", correcting: "tx-existing" }),
    );
  });

  // Submit calls correctTransaction with correct args: (id, accountId, dto)
  it("calls correctTransaction with correct args on submit", async () => {
    mockCorrectTransaction.mockResolvedValue({
      data: { id: "tx-existing" },
      error: null,
    });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() =>
      useEditTransactionModal({
        transaction: baseTransaction,
        onSubmitSuccess,
      }),
    );
    await waitFor(() => expect(result.current.isFormValid).toBe(true));

    const fakeSubmit = {
      preventDefault: vi.fn(),
    } as unknown as React.FormEvent;

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockCorrectTransaction).toHaveBeenCalledWith(
      "tx-existing",
      "account-1",
      expect.objectContaining({
        quantity: 2 * MICRO,
        unit_price: 50 * MICRO,
        exchange_rate: 1 * MICRO,
      }),
    );
    expect(onSubmitSuccess).toHaveBeenCalledTimes(1);
  });

  // Backend error stays modal open
  it("sets error and does not call onSubmitSuccess on backend error", async () => {
    mockCorrectTransaction.mockResolvedValue({
      data: null,
      error: "Not found",
    });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() =>
      useEditTransactionModal({
        transaction: baseTransaction,
        onSubmitSuccess,
      }),
    );
    await waitFor(() => expect(result.current.isFormValid).toBe(true));

    const fakeSubmit = {
      preventDefault: vi.fn(),
    } as unknown as React.FormEvent;
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toBe("Not found");
    expect(onSubmitSuccess).not.toHaveBeenCalled();
  });

  // MKT-052 — recordPrice is always false on edit regardless of localStorage
  it("recordPrice is false on edit mount even when localStorage auto_record_price is true", () => {
    localStorage.setItem("auto_record_price", "true");
    const { result } = renderHook(() => useEditTransactionModal({ transaction: baseTransaction }));
    expect(result.current.recordPrice).toBe(false);
  });

  // MKT-054 — calls recordAssetPrice when recordPrice is manually set to true
  it("calls recordAssetPrice when recordPrice is manually set to true", async () => {
    mockCorrectTransaction.mockResolvedValue({
      data: { id: "tx-existing" },
      error: null,
    });
    mockRecordAssetPrice.mockResolvedValue({ status: "ok", data: null });

    const { result } = renderHook(() => useEditTransactionModal({ transaction: baseTransaction }));

    await act(async () => {
      result.current.setRecordPrice(true);
    });

    const fakeSubmit = {
      preventDefault: vi.fn(),
    } as unknown as React.FormEvent;
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordAssetPrice).toHaveBeenCalledWith("asset-1", "2024-01-10", 50);
  });

  // MKT-061 — skip recordAssetPrice when recordPrice is true but unit_price is 0
  it("does not call recordAssetPrice when recordPrice is true but unit_price is 0", async () => {
    mockCorrectTransaction.mockResolvedValue({
      data: { id: "tx-zero-price" },
      error: null,
    });

    const { result } = renderHook(() =>
      useEditTransactionModal({ transaction: zeroUnitPriceTransaction }),
    );

    await act(async () => {
      result.current.setRecordPrice(true);
    });

    const fakeSubmit = {
      preventDefault: vi.fn(),
    } as unknown as React.FormEvent;
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockCorrectTransaction).toHaveBeenCalledWith(
      "tx-zero-price",
      "account-1",
      expect.objectContaining({ unit_price: 0 }),
    );
    expect(mockRecordAssetPrice).not.toHaveBeenCalled();
  });

  // TRX-063 / DIV-040 — a corrected dividend is checked by the core as a dividend, and its
  // total on screen is the core's, not a figure computed here.
  it("shows the core's total for a corrected dividend", async () => {
    mockValidateDraft.mockResolvedValue({
      status: "ok",
      data: { unit_price: 1 * MICRO, total_amount: 13_750_000 },
    });
    const dividend: Transaction = {
      ...baseTransaction,
      transaction_type: "Dividend",
      quantity: 12_500_000,
      unit_price: 1 * MICRO,
      exchange_rate: 1_100_000,
      total_amount: 13_750_000,
    };
    const { result } = renderHook(() => useEditTransactionModal({ transaction: dividend }));
    await act(async () => {});

    expect(mockValidateDraft).toHaveBeenLastCalledWith(
      expect.objectContaining({
        kind: "Dividend",
        quantity: 12_500_000,
        correcting: "tx-existing",
      }),
    );
    expect(result.current.totalAmountDisplay).toBe(microToFormatted(13_750_000));
    expect(result.current.isFormValid).toBe(true);

    mockValidateDraft.mockResolvedValue({
      status: "ok",
      data: { unit_price: 1 * MICRO, total_amount: 15_000_000 },
    });
    await act(async () => result.current.handleChange("exchangeRate", "1.2"));
    expect(result.current.totalAmountDisplay).toBe(microToFormatted(15_000_000));

    mockValidateDraft.mockResolvedValue({
      status: "error",
      error: { code: "ExchangeRateNotPositive" },
    });
    await act(async () => result.current.handleChange("exchangeRate", "0"));
    expect(result.current.isFormValid).toBe(false);
    expect(result.current.fieldErrors.exchangeRate).toBeDefined();
  });

  // TRX-051: OpeningBalance pre-fill uses total_amount (not unit_price) as the "Total Cost" field
  it("TRX-051: pre-fills unitPrice from total_amount for OpeningBalance", () => {
    const { result } = renderHook(() =>
      useEditTransactionModal({ transaction: openingBalanceTransaction }),
    );
    expect(result.current.formData.unitPrice).toBe("100.000");
  });

  // TRX-051 / TD-033: submit sends the typed total cost; the backend derives the unit price
  it("TRX-051: submit sends the typed total cost, zero fees, unit exchange_rate, null note", async () => {
    mockCorrectTransaction.mockResolvedValue({
      data: { id: "tx-ob" },
      error: null,
    });
    const { result } = renderHook(() =>
      useEditTransactionModal({ transaction: openingBalanceTransaction }),
    );

    const fakeSubmit = {
      preventDefault: vi.fn(),
    } as unknown as React.FormEvent;
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockCorrectTransaction).toHaveBeenCalledWith(
      "tx-ob",
      "account-1",
      expect.objectContaining({
        total_amount: 100 * MICRO,
        unit_price: 0,
        exchange_rate: 1 * MICRO,
        fees: 0,
        note: null,
      }),
    );
  });

  // MKT-062 — recordAssetPrice failure is silent; edit commits and onSubmitSuccess fires
  it("swallows recordAssetPrice rejection and still calls onSubmitSuccess", async () => {
    mockCorrectTransaction.mockResolvedValue({
      data: { id: "tx-existing" },
      error: null,
    });
    mockRecordAssetPrice.mockRejectedValue(new Error("network error"));

    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() =>
      useEditTransactionModal({
        transaction: baseTransaction,
        onSubmitSuccess,
      }),
    );

    await act(async () => {
      result.current.setRecordPrice(true);
    });

    const fakeSubmit = {
      preventDefault: vi.fn(),
    } as unknown as React.FormEvent;
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toBeNull();
    expect(onSubmitSuccess).toHaveBeenCalledTimes(1);
  });

  // TRX-061 / SEL-051 — total-entry correction.
  describe("total-entry mode", () => {
    const sellTransaction: Transaction = {
      ...baseTransaction,
      id: "tx-sell",
      transaction_type: "Sell",
    };

    const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

    // TRX-051 — an opening balance's total is its typed total cost, not quantity × cost
    it("shows an opening balance's total cost as typed", () => {
      const { result } = renderHook(() =>
        useEditTransactionModal({ transaction: openingBalanceTransaction }),
      );
      expect(result.current.totalAmountDisplay).toBe("100,000");
    });

    it("offers total-entry for Purchase and Sell but not OpeningBalance", async () => {
      const purchase = renderHook(() => useEditTransactionModal({ transaction: baseTransaction }));
      const sell = renderHook(() => useEditTransactionModal({ transaction: sellTransaction }));
      const ob = renderHook(() =>
        useEditTransactionModal({ transaction: openingBalanceTransaction }),
      );
      expect(purchase.result.current.isTotalEntryEligible).toBe(true);
      expect(sell.result.current.isTotalEntryEligible).toBe(true);
      expect(ob.result.current.isTotalEntryEligible).toBe(false);
      await waitFor(() => expect(sell.result.current.isFormValid).toBe(true));
      expect(mockValidateDraft).toHaveBeenCalledWith(
        expect.objectContaining({ kind: "Sell", correcting: "tx-sell" }),
      );
      // TRX-063 — an opening balance is checked on save, not by the draft check
      expect(mockValidateDraft).not.toHaveBeenCalledWith(
        expect.objectContaining({ correcting: "tx-ob" }),
      );
      expect(ob.result.current.isFormValid).toBe(true);
    });

    it("price mode (default) submits total_amount: null", async () => {
      mockCorrectTransaction.mockResolvedValue({ data: { id: "tx-existing" }, error: null });
      const { result } = renderHook(() =>
        useEditTransactionModal({ transaction: baseTransaction }),
      );
      await waitFor(() => expect(result.current.isFormValid).toBe(true));
      await act(async () => {
        await result.current.handleSubmit(fakeSubmit);
      });
      expect(mockCorrectTransaction).toHaveBeenCalledWith(
        "tx-existing",
        "account-1",
        expect.objectContaining({ total_amount: null }),
      );
    });

    // TRX-061 — the typed purchase total is stored verbatim; unit price is derived.
    it("purchase total mode ships the typed total and a derived unit price", async () => {
      mockValidateDraft.mockImplementation(
        answersByMode([50 * MICRO, 100 * MICRO], [55 * MICRO, 110 * MICRO]),
      );
      mockCorrectTransaction.mockResolvedValue({ data: { id: "tx-existing" }, error: null });
      const { result } = renderHook(() =>
        useEditTransactionModal({ transaction: baseTransaction }),
      );
      // Switch to total mode (seeds from computed 100), then type an all-in 110.
      await act(async () => {
        result.current.handleEntryModeChange("total");
      });
      await act(async () => {
        result.current.handleTotalAmountChange("110");
      });
      await act(async () => {
        await result.current.handleSubmit(fakeSubmit);
      });
      expect(mockCorrectTransaction).toHaveBeenCalledWith(
        "tx-existing",
        "account-1",
        expect.objectContaining({ total_amount: 110 * MICRO, unit_price: 55 * MICRO }),
      );
    });

    // SEL-051 — the sell total is net proceeds; fees are added back to derive the unit price.
    it("sell total mode derives unit price from net proceeds plus fees", async () => {
      mockValidateDraft.mockImplementation(
        answersByMode([50 * MICRO, 90 * MICRO], [50 * MICRO, 90 * MICRO]),
      );
      mockCorrectTransaction.mockResolvedValue({ data: { id: "tx-sell" }, error: null });
      const { result } = renderHook(() =>
        useEditTransactionModal({ transaction: sellTransaction }),
      );
      await act(async () => {
        result.current.handleChange("fees", "10");
      });
      await act(async () => {
        result.current.handleEntryModeChange("total");
      });
      await act(async () => {
        result.current.handleTotalAmountChange("90");
      });
      await act(async () => {
        await result.current.handleSubmit(fakeSubmit);
      });
      expect(mockCorrectTransaction).toHaveBeenCalledWith(
        "tx-sell",
        "account-1",
        expect.objectContaining({ total_amount: 90 * MICRO, unit_price: 50 * MICRO }),
      );
    });

    // TRX-061 — switching back to price mode seeds the unit price the check derived
    it("seeds the unit-price field from the check when switching back to price mode", async () => {
      mockValidateDraft.mockImplementation(
        answersByMode([50 * MICRO, 100 * MICRO], [55 * MICRO, 110 * MICRO]),
      );
      const { result } = renderHook(() =>
        useEditTransactionModal({ transaction: baseTransaction }),
      );
      await act(async () => {
        result.current.handleEntryModeChange("total");
      });
      await act(async () => {
        result.current.handleTotalAmountChange("110");
      });
      await waitFor(() => expect(result.current.unitPriceDisplay).toBe("55,000"));
      await act(async () => {
        result.current.handleEntryModeChange("price");
      });
      expect(result.current.formData.unitPrice).toBe("55.000");
    });

    // TRX-063 — a correction with a problem is not sent; the problem becomes the error
    it("does not send a correction the check rejects", async () => {
      mockValidateDraft.mockResolvedValue({
        status: "error",
        error: { code: "QuantityNotPositive" },
      });
      const { result } = renderHook(() =>
        useEditTransactionModal({ transaction: baseTransaction }),
      );
      await waitFor(() => expect(mockValidateDraft).toHaveBeenCalled());
      await act(async () => {}); // the check's answer lands
      await act(async () => {
        await result.current.handleSubmit(fakeSubmit);
      });
      expect(mockCorrectTransaction).not.toHaveBeenCalled();
      expect(result.current.error).toEqual({ key: "error.QuantityNotPositive" });
    });

    // TRX-060 — a purchase total below its included fees is rejected inline.
    it("flags a purchase total below fees and blocks submit", async () => {
      mockValidateDraft.mockImplementation(async (draft: TransactionDraft) =>
        draft.entered.mode === "Total" && draft.entered.total < draft.entered.fees
          ? { status: "error", error: { code: "TotalAmountBelowFees" } }
          : fakeDraftCheck(draft),
      );
      const { result } = renderHook(() =>
        useEditTransactionModal({ transaction: baseTransaction }),
      );
      await act(async () => {
        result.current.handleChange("fees", "20");
      });
      await act(async () => {
        result.current.handleEntryModeChange("total");
      });
      await act(async () => {
        result.current.handleTotalAmountChange("10");
      });
      expect(result.current.fieldErrors.total).toBeDefined();
      expect(result.current.problemHint).toBeNull();
      expect(result.current.isFormValid).toBe(false);
    });
  });
});
