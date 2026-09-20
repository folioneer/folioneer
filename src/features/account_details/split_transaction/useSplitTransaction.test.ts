import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { logger } from "@/lib/logger";
import { useSplitTransaction } from "./useSplitTransaction";

// ── Hoisted mocks ──────────────────────────────────────────────────────────────
const { mockRecordSplit, mockCorrectTransaction, mockRecordAssetPrice, mockShowSnackbar } =
  vi.hoisted(() => ({
    mockRecordSplit: vi.fn(),
    mockCorrectTransaction: vi.fn(),
    mockRecordAssetPrice: vi.fn(),
    mockShowSnackbar: vi.fn(),
  }));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    recordSplit: mockRecordSplit,
    correctTransaction: mockCorrectTransaction,
    recordAssetPrice: mockRecordAssetPrice,
  },
}));

vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en" },
  }),
}));

// ── Fixtures ───────────────────────────────────────────────────────────────────
const TODAY = new Date().toISOString().slice(0, 10);

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

// 10 shares at an average of 150.00, latest price 150.00.
const target = {
  assetId: "asset-equity-1",
  assetName: "Alphabet Inc",
  holdingQuantityMicro: 10_000_000,
  averagePriceMicro: 150_000_000,
  currentPriceMicro: 150_000_000,
};

const unpricedTarget = { ...target, currentPriceMicro: null };

const BASE_PROPS = {
  accountId: "account-1",
  target,
  onSubmitSuccess: vi.fn(),
};

// ── Tests ──────────────────────────────────────────────────────────────────────
describe("useSplitTransaction — create mode (SPL-061/040)", () => {
  beforeEach(() => {
    mockRecordSplit.mockReset();
    mockCorrectTransaction.mockReset();
    mockRecordAssetPrice.mockReset();
    mockShowSnackbar.mockReset();
    vi.mocked(logger.error).mockClear();
    vi.mocked(logger.warn).mockClear();
    BASE_PROPS.onSubmitSuccess.mockClear();
  });

  // SPL-061 — initial form: today's date, default 2 : 1 ratio, blank note
  it("initial state has today's date, a 2 : 1 ratio, and a blank note", () => {
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    expect(result.current.formData.date).toBe(TODAY);
    expect(result.current.formData.ratioNew).toBe("2");
    expect(result.current.formData.ratioOld).toBe("1");
    expect(result.current.formData.note).toBe("");
    expect(result.current.isFormValid).toBe(true);
  });

  // SPL-061 — preview mirrors the SPL-020 formulas: qty ×2, average halved
  it("previews the rescaled quantity and average price for the default 2 : 1 ratio", () => {
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    expect(result.current.preview).toEqual(
      expect.objectContaining({
        oldQuantity: "10",
        // Formatted through the shared micro formatters (default "fr" locale in tests).
        oldAveragePrice: "150,00",
        newQuantity: "20",
        newAveragePrice: "75,00",
        newQuantityMicro: 20_000_000,
      }),
    );
  });

  // SPL-011 — ratio invalid when a part is not a positive integer
  it("flags the ratio and disables submit when a ratio part is zero", () => {
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    act(() => result.current.handleChange("ratioOld", "0"));
    expect(result.current.ratioError).toEqual({
      key: "transaction.error_validation_split_ratio",
    });
    expect(result.current.isFormValid).toBe(false);
  });

  // SPL-011 — a ×1 split is a no-op data-entry error
  it("flags the ratio and disables submit for a 1 : 1 ratio (factor = 1)", () => {
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    act(() => result.current.handleChange("ratioNew", "1"));
    expect(result.current.ratioError).toEqual({
      key: "transaction.error_validation_split_ratio",
    });
    expect(result.current.isFormValid).toBe(false);
  });

  // SPL-021 — a reverse split flooring the quantity to zero disables submit
  it("disables submit when the preview quantity floors to zero (collapse guard)", () => {
    const tinyTarget = { ...target, holdingQuantityMicro: 1 };
    const props = { ...BASE_PROPS, target: tinyTarget };
    const { result } = renderHook(() => useSplitTransaction(props));
    act(() => {
      result.current.handleChange("ratioNew", "1");
      result.current.handleChange("ratioOld", "2");
    });
    expect(result.current.collapsesPosition).toBe(true);
    expect(result.current.isFormValid).toBe(false);
  });

  // SPL-061 — factor = round(new × MICRO / old): 3 : 2 → 1_500_000
  it("submits the micro-scaled factor computed from the ratio pair", async () => {
    mockRecordSplit.mockResolvedValue({ status: "ok", data: { id: "tx-spl-1" } });
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("ratioNew", "3");
      result.current.handleChange("ratioOld", "2");
      result.current.setRecordPrice(false);
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordSplit).toHaveBeenCalledWith({
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: TODAY,
      factor: 1_500_000,
      note: null,
    });
    expect(mockShowSnackbar).toHaveBeenCalledWith("split.recorded", "success");
    expect(BASE_PROPS.onSubmitSuccess).toHaveBeenCalled();
  });

  // SPL-040 — post-split price prefilled as round(latest price × MICRO / factor)
  it("prefills the derived post-split price and records it best-effort on success", async () => {
    mockRecordSplit.mockResolvedValue({ status: "ok", data: { id: "tx-spl-1" } });
    mockRecordAssetPrice.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));

    // Latest price 150.00 with the default 2 : 1 factor → 75.00.
    expect(result.current.recordPrice).toBe(true);
    expect(result.current.priceInput).toBe("75.000");

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordAssetPrice).toHaveBeenCalledWith("asset-equity-1", TODAY, 75);
  });

  // SPL-040 — price recording is best-effort: a failure never fails the submit
  it("keeps the success flow when the post-split price record rejects (best-effort)", async () => {
    mockRecordSplit.mockResolvedValue({ status: "ok", data: { id: "tx-spl-1" } });
    mockRecordAssetPrice.mockRejectedValue(new Error("ipc broken"));
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("split.recorded", "success");
    expect(BASE_PROPS.onSubmitSuccess).toHaveBeenCalled();
    await waitFor(() => expect(logger.warn).toHaveBeenCalled());
  });

  // SPL-040 — without a prior price: checkbox unchecked, field empty, no record
  it("starts unchecked with an empty price and skips the record when no prior price exists", async () => {
    mockRecordSplit.mockResolvedValue({ status: "ok", data: { id: "tx-spl-1" } });
    const props = { ...BASE_PROPS, target: unpricedTarget };
    const { result } = renderHook(() => useSplitTransaction(props));

    expect(result.current.recordPrice).toBe(false);
    expect(result.current.priceInput).toBe("");

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordAssetPrice).not.toHaveBeenCalled();
  });

  // SPL-040 — the user can edit the derived price; the typed value is recorded
  it("records the user-edited price instead of the derived prefill", async () => {
    mockRecordSplit.mockResolvedValue({ status: "ok", data: { id: "tx-spl-1" } });
    mockRecordAssetPrice.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));

    act(() => result.current.handlePriceChange("74.5"));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordAssetPrice).toHaveBeenCalledWith("asset-equity-1", TODAY, 74.5);
  });

  // F27 — backend rejection surfaces as an inline typed error via the presenter
  it("surfaces a backend error code as an inline error (F27)", async () => {
    mockRecordSplit.mockResolvedValue({
      status: "error",
      error: { code: "SplitCollapsesPosition" },
    });
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.SplitCollapsesPosition" });
    expect(mockShowSnackbar).not.toHaveBeenCalled();
    expect(logger.error).toHaveBeenCalled();
  });

  // SPL-011 — an invalid ratio blocks the submit before any gateway call
  it("does not call the gateway when the ratio is invalid", async () => {
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    act(() => result.current.handleChange("ratioNew", ""));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordSplit).not.toHaveBeenCalled();
    expect(result.current.error).toEqual({
      key: "transaction.error_validation_split_ratio",
    });
  });
});

describe("useSplitTransaction — edit mode (SPL-030)", () => {
  const editProps = {
    ...BASE_PROPS,
    editMode: {
      transactionId: "tx-spl-existing",
      lockedAssetId: "asset-equity-1",
      lockedAssetName: "Alphabet Inc",
      initialDate: "2024-06-15",
      initialFactor: "2.000",
      initialNote: "20-for-10",
    },
  };

  beforeEach(() => {
    mockRecordSplit.mockReset();
    mockCorrectTransaction.mockReset();
    mockRecordAssetPrice.mockReset();
    mockShowSnackbar.mockReset();
    BASE_PROPS.onSubmitSuccess.mockClear();
  });

  // SPL-030 — the form prefills from the transaction (factor as a decimal multiplier)
  it("prefills date, factor, and note from the transaction", () => {
    const { result } = renderHook(() => useSplitTransaction(editProps));
    expect(result.current.isEditMode).toBe(true);
    expect(result.current.formData.date).toBe("2024-06-15");
    expect(result.current.formData.factor).toBe("2.000");
    expect(result.current.formData.note).toBe("20-for-10");
    // No preview and no price checkbox in edit mode (SPL-030).
    expect(result.current.preview).toBeNull();
    expect(result.current.recordPrice).toBe(false);
  });

  // SPL-030 — the correction rides correct_transaction with the factor in `quantity`
  it("submits via correctTransaction with the factor in the quantity field", async () => {
    mockCorrectTransaction.mockResolvedValue({ status: "ok", data: {} });
    const { result } = renderHook(() => useSplitTransaction(editProps));

    act(() => result.current.handleChange("factor", "3"));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockCorrectTransaction).toHaveBeenCalledWith("tx-spl-existing", "account-1", {
      date: "2024-06-15",
      quantity: 3_000_000,
      unit_price: 0,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: null,
      note: "20-for-10",
    });
    expect(mockRecordSplit).not.toHaveBeenCalled();
    // No post-split price record on the edit path (SPL-030).
    expect(mockRecordAssetPrice).not.toHaveBeenCalled();
    expect(mockShowSnackbar).toHaveBeenCalledWith("split.updated", "success");
  });

  // SPL-011 — a ×1 factor is rejected in edit mode too
  it("flags a factor of 1 and blocks the submit", async () => {
    const { result } = renderHook(() => useSplitTransaction(editProps));
    act(() => result.current.handleChange("factor", "1"));
    expect(result.current.isFormValid).toBe(false);

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockCorrectTransaction).not.toHaveBeenCalled();
  });
});
