import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { logger } from "@/lib/logger";
import { useSplitTransaction } from "./useSplitTransaction";

// ── Hoisted mocks ──────────────────────────────────────────────────────────────
const {
  mockRecordSplit,
  mockCorrectTransaction,
  mockRecordAssetPrice,
  mockShowSnackbar,
  mockValidateStockSplitDraft,
} = vi.hoisted(() => ({
  mockValidateStockSplitDraft: vi.fn(),
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
    validateStockSplitDraft: mockValidateStockSplitDraft,
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

// A holding whose latest price is 150.00.
const target = {
  assetId: "asset-equity-1",
  assetName: "Alphabet Inc",
  currentPriceMicro: 150_000_000,
};

const unpricedTarget = { ...target, currentPriceMicro: null };

const BASE_PROPS = {
  accountId: "account-1",
  target,
  onSubmitSuccess: vi.fn(),
};

// The split draft check (SPL-062) is the core's. This stand-in answers a ratio draft with
// a fixed factor, position and price, so a test can tell what the form shows from what it
// would have computed itself.
const answer = (overrides: Record<string, unknown> = {}) => ({
  status: "ok",
  data: {
    factor: 2_000_000,
    position: {
      old_quantity: 10_000_000,
      old_average_price: 150_000_000,
      new_quantity: 20_000_000,
      new_average_price: 75_000_000,
    },
    price_after_split: 75_000_000,
    ...overrides,
  },
});
const problem = (code: string) => ({ status: "error", error: { code } });

const resetMocks = () => {
  mockRecordSplit.mockReset();
  mockCorrectTransaction.mockReset();
  mockRecordAssetPrice.mockReset().mockResolvedValue({ status: "ok", data: null });
  mockShowSnackbar.mockReset();
  mockValidateStockSplitDraft.mockReset().mockResolvedValue(answer());
  vi.mocked(logger.error).mockClear();
  vi.mocked(logger.warn).mockClear();
  BASE_PROPS.onSubmitSuccess.mockClear();
};

// ── Tests ──────────────────────────────────────────────────────────────────
describe("useSplitTransaction — create mode (SPL-061/062/040)", () => {
  beforeEach(resetMocks);

  // SPL-061 — initial form: today's date, default 2 : 1 ratio, blank note
  it("initial state has today's date, a 2 : 1 ratio, and a blank note", () => {
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    expect(result.current.formData.date).toBe(TODAY);
    expect(result.current.formData.ratioNew).toBe("2");
    expect(result.current.formData.ratioOld).toBe("1");
    expect(result.current.formData.note).toBe("");
    expect(result.current.isEditMode).toBe(false);
  });

  // SPL-062 — the form sends the ratio as typed and shows what the core returns: the
  // position before and after, and the price to carry across the split.
  it("sends the ratio as typed and shows the core's preview", async () => {
    mockValidateStockSplitDraft.mockResolvedValue(
      answer({
        factor: 1_500_000,
        position: {
          old_quantity: 7_000_000,
          old_average_price: 33_476_190,
          new_quantity: 10_500_000,
          new_average_price: 22_317_460,
        },
        price_after_split: 66_666_667,
      }),
    );
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    act(() => result.current.handleChange("ratioNew", "3"));
    act(() => result.current.handleChange("ratioOld", "2"));

    await waitFor(() => expect(result.current.preview).not.toBeNull());
    expect(mockValidateStockSplitDraft).toHaveBeenLastCalledWith({
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: TODAY,
      size: { mode: "Ratio", new: 3, old: 2 },
      correcting: null,
    });
    // Not the holding the dialog was opened on (10 @ 150): the figures are the core's.
    expect(result.current.preview?.oldQuantity).toContain("7");
    expect(result.current.priceInput).toBe("66.666667");
    expect(result.current.isFormValid).toBe(true);
  });

  // SPL-061 — a ratio part that is not a whole number is sent as not typed yet.
  it("sends a ratio part that is not a whole number as missing", async () => {
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    act(() => result.current.handleChange("ratioNew", "2.5"));
    await waitFor(() =>
      expect(mockValidateStockSplitDraft).toHaveBeenLastCalledWith(
        expect.objectContaining({ size: { mode: "Ratio", new: null, old: 1 } }),
      ),
    );
  });

  // SPL-011 — the core refuses a factor that is not positive or is 1: the ratio is
  // flagged and nothing can be saved.
  it.each([
    "SplitFactorNotPositive",
    "SplitFactorIsOne",
  ])("flags the ratio when the core answers %s", async (code) => {
    mockValidateStockSplitDraft.mockResolvedValue(problem(code));
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    await waitFor(() => expect(result.current.ratioError).not.toBeNull());
    expect(result.current.isFormValid).toBe(false);
    expect(result.current.preview).toBeNull();
    expect(result.current.error).toBeNull();
  });

  // SPL-021 — a split the core says leaves nothing is stated, with its own message.
  it("states a split the core says leaves nothing", async () => {
    mockValidateStockSplitDraft.mockResolvedValue(problem("SplitCollapsesPosition"));
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    await waitFor(() =>
      expect(result.current.error).toEqual({ key: "error.SplitCollapsesPosition" }),
    );
    expect(result.current.isFormValid).toBe(false);
    expect(result.current.ratioError).toBeNull();
  });

  // A note is no part of the check: typing one asks the core nothing.
  it("does not check again when the note changes", async () => {
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    await waitFor(() => expect(result.current.isFormValid).toBe(true));
    const asked = mockValidateStockSplitDraft.mock.calls.length;
    act(() => result.current.handleChange("note", "2 for 1"));
    expect(result.current.isFormValid).toBe(true);
    expect(mockValidateStockSplitDraft).toHaveBeenCalledTimes(asked);
  });

  // TRX-067 — any other problem (a date, a position not held then) is said, not only
  // shown by a disabled button; so is a check that could not run.
  it("says why when the core reports another problem or the check fails", async () => {
    mockValidateStockSplitDraft.mockResolvedValue(problem("ClosedPosition"));
    const closed = renderHook(() => useSplitTransaction(BASE_PROPS));
    await waitFor(() => expect(closed.result.current.error).not.toBeNull());
    expect(closed.result.current.isFormValid).toBe(false);
    closed.unmount();

    mockValidateStockSplitDraft.mockRejectedValue(new Error("ipc down"));
    const failed = renderHook(() => useSplitTransaction(BASE_PROPS));
    await waitFor(() => expect(failed.result.current.error).toEqual({ key: "error.Unknown" }));
    expect(failed.result.current.isFormValid).toBe(false);
  });

  // SPL-061 — the factor recorded is the one the core returned for the ratio.
  it("records the factor the core returned", async () => {
    mockValidateStockSplitDraft.mockResolvedValue(answer({ factor: 333_333 }));
    mockRecordSplit.mockResolvedValue({ status: "ok", data: {} });
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    act(() => result.current.handleChange("ratioNew", "1"));
    act(() => result.current.handleChange("ratioOld", "3"));
    act(() => result.current.handleChange("note", "Reverse split"));
    await waitFor(() => expect(result.current.isFormValid).toBe(true));

    await act(async () => result.current.handleSubmit(fakeSubmit));

    expect(mockRecordSplit).toHaveBeenCalledWith({
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: TODAY,
      factor: 333_333,
      note: "Reverse split",
    });
    expect(mockShowSnackbar).toHaveBeenCalledWith("split.recorded", "success");
    expect(BASE_PROPS.onSubmitSuccess).toHaveBeenCalledTimes(1);
  });

  // SPL-040 — the price field is prefilled with the core's price and recorded, best
  // effort, after the split.
  it("prefills the core's post-split price and records it best-effort on success", async () => {
    mockRecordSplit.mockResolvedValue({ status: "ok", data: {} });
    mockRecordAssetPrice.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    await waitFor(() => expect(result.current.priceInput).toBe("75.000"));
    expect(result.current.recordPrice).toBe(true);

    await act(async () => result.current.handleSubmit(fakeSubmit));

    expect(mockRecordAssetPrice).toHaveBeenCalledWith("asset-equity-1", TODAY, 75);
  });

  it("keeps the success flow when the post-split price record rejects (best-effort)", async () => {
    mockRecordSplit.mockResolvedValue({ status: "ok", data: {} });
    mockRecordAssetPrice.mockRejectedValue(new Error("price write failed"));
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    await waitFor(() => expect(result.current.isFormValid).toBe(true));

    await act(async () => result.current.handleSubmit(fakeSubmit));

    expect(mockShowSnackbar).toHaveBeenCalledWith("split.recorded", "success");
    await waitFor(() => expect(logger.warn).toHaveBeenCalled());
  });

  // SPL-040 — without a price there is nothing to prefill nor to record.
  it("starts unchecked with an empty price and skips the record when the asset has no price", async () => {
    mockValidateStockSplitDraft.mockResolvedValue(answer({ price_after_split: null }));
    mockRecordSplit.mockResolvedValue({ status: "ok", data: {} });
    const { result } = renderHook(() =>
      useSplitTransaction({ ...BASE_PROPS, target: unpricedTarget }),
    );
    await waitFor(() => expect(result.current.isFormValid).toBe(true));
    expect(result.current.recordPrice).toBe(false);
    expect(result.current.priceInput).toBe("");

    await act(async () => result.current.handleSubmit(fakeSubmit));

    expect(mockRecordAssetPrice).not.toHaveBeenCalled();
  });

  it("records the user-edited price instead of the prefill", async () => {
    mockRecordSplit.mockResolvedValue({ status: "ok", data: {} });
    mockRecordAssetPrice.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    await waitFor(() => expect(result.current.isFormValid).toBe(true));
    act(() => result.current.handlePriceChange("80"));

    await act(async () => result.current.handleSubmit(fakeSubmit));

    expect(mockRecordAssetPrice).toHaveBeenCalledWith("asset-equity-1", TODAY, 80);
  });

  // F27 — a refusal on save is shown and the dialog stays open.
  it("surfaces a backend error code as an inline error (F27)", async () => {
    mockRecordSplit.mockResolvedValue({ status: "error", error: { code: "ClosedPosition" } });
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    await waitFor(() => expect(result.current.isFormValid).toBe(true));

    await act(async () => result.current.handleSubmit(fakeSubmit));

    expect(result.current.error).not.toBeNull();
    expect(BASE_PROPS.onSubmitSuccess).not.toHaveBeenCalled();
    expect(logger.error).toHaveBeenCalled();
  });

  // Saving follows the check: while the core reports a problem, nothing is recorded.
  it("records nothing while the check is not clean", async () => {
    mockValidateStockSplitDraft.mockResolvedValue(problem("SplitFactorIsOne"));
    const { result } = renderHook(() => useSplitTransaction(BASE_PROPS));
    await waitFor(() => expect(result.current.ratioError).not.toBeNull());

    await act(async () => result.current.handleSubmit(fakeSubmit));

    expect(mockRecordSplit).not.toHaveBeenCalled();
  });
});

describe("useSplitTransaction — edit mode (SPL-030)", () => {
  const editMode = {
    transactionId: "tx-split-1",
    lockedAssetId: "asset-equity-1",
    lockedAssetName: "Alphabet Inc",
    initialDate: "2024-06-15",
    initialFactor: "2.000",
    initialNote: "2-for-1",
  };
  const EDIT_PROPS = { ...BASE_PROPS, editMode };

  beforeEach(() => {
    resetMocks();
    mockValidateStockSplitDraft.mockResolvedValue(answer({ position: null }));
  });

  it("prefills date, factor, and note from the transaction", () => {
    const { result } = renderHook(() => useSplitTransaction(EDIT_PROPS));
    expect(result.current.isEditMode).toBe(true);
    expect(result.current.formData.date).toBe("2024-06-15");
    expect(result.current.formData.factor).toBe("2.000");
    expect(result.current.formData.note).toBe("2-for-1");
    expect(result.current.recordPrice).toBe(false);
  });

  // SPL-030 — a correction sends its factor and what it corrects; no position is shown.
  it("checks the corrected split by its factor and shows no position", async () => {
    const { result } = renderHook(() => useSplitTransaction(EDIT_PROPS));
    await waitFor(() => expect(result.current.isFormValid).toBe(true));

    expect(mockValidateStockSplitDraft).toHaveBeenLastCalledWith({
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: "2024-06-15",
      size: { mode: "Factor", factor: 2_000_000 },
      correcting: "tx-split-1",
    });
    expect(result.current.preview).toBeNull();
  });

  it("submits via correctTransaction with the factor in the quantity field", async () => {
    mockValidateStockSplitDraft.mockResolvedValue(answer({ factor: 3_000_000, position: null }));
    mockCorrectTransaction.mockResolvedValue({ status: "ok", data: {} });
    const { result } = renderHook(() => useSplitTransaction(EDIT_PROPS));
    act(() => result.current.handleChange("factor", "3"));
    await waitFor(() => expect(result.current.isFormValid).toBe(true));

    await act(async () => result.current.handleSubmit(fakeSubmit));

    expect(mockCorrectTransaction).toHaveBeenCalledWith("tx-split-1", "account-1", {
      date: "2024-06-15",
      quantity: 3_000_000,
      unit_price: 0,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: null,
      note: "2-for-1",
    });
    expect(mockRecordAssetPrice).not.toHaveBeenCalled();
    expect(mockShowSnackbar).toHaveBeenCalledWith("split.updated", "success");
  });

  it("flags a factor the core refuses and blocks the submit", async () => {
    mockValidateStockSplitDraft.mockResolvedValue(problem("SplitFactorIsOne"));
    const { result } = renderHook(() => useSplitTransaction(EDIT_PROPS));
    act(() => result.current.handleChange("factor", "1"));
    await waitFor(() => expect(result.current.ratioError).not.toBeNull());

    await act(async () => result.current.handleSubmit(fakeSubmit));

    expect(mockCorrectTransaction).not.toHaveBeenCalled();
  });
});
