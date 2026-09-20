import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { logger } from "@/lib/logger";
import { useInterestTransaction } from "./useInterestTransaction";

// ── Hoisted mocks ──────────────────────────────────────────────────────────────
const { mockRecordInterest, mockCorrectTransaction, mockShowSnackbar } = vi.hoisted(() => ({
  mockRecordInterest: vi.fn(),
  mockCorrectTransaction: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    recordInterest: mockRecordInterest,
    correctTransaction: mockCorrectTransaction,
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

const BASE_PROPS = {
  accountId: "account-1",
  onSubmitSuccess: vi.fn(),
};

// ── Tests ──────────────────────────────────────────────────────────────────────
describe("useInterestTransaction (INT-020/021/025)", () => {
  beforeEach(() => {
    mockRecordInterest.mockReset();
    mockCorrectTransaction.mockReset();
    mockShowSnackbar.mockReset();
    vi.mocked(logger.error).mockClear();
    BASE_PROPS.onSubmitSuccess.mockClear();
  });

  // INT-020 — initial form has today's date, no asset, blank percent/quantity/note
  it("initial state has today's date, no asset selected, blank percent, quantity, and note", () => {
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));
    expect(result.current.formData.date).toBe(TODAY);
    expect(result.current.formData.assetId).toBe("");
    expect(result.current.formData.percent).toBe("");
    expect(result.current.formData.quantity).toBe("");
    expect(result.current.formData.note).toBe("");
  });

  // INT-021 — form invalid when no asset selected
  it("isFormValid false when no asset is selected", () => {
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));
    act(() => result.current.handleChange("percent", "2"));
    expect(result.current.isFormValid).toBe(false);
  });

  // INT-021 — form invalid when neither percent nor quantity is filled
  it("isFormValid false when neither percent nor quantity is filled", () => {
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));
    act(() => result.current.handleChange("assetId", "asset-fund-1"));
    expect(result.current.isFormValid).toBe(false);
  });

  // INT-021 — both fields filled keeps the submit gate open so handleSubmit can
  // surface the InterestAmountInvalid message (a disabled button would hide it).
  it("keeps the gate open with both fields filled and surfaces InterestAmountInvalid on submit", async () => {
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));
    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
      result.current.handleChange("percent", "2");
      result.current.handleChange("quantity", "10");
    });
    expect(result.current.isFormValid).toBe(true);
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(result.current.error).toEqual({ key: "error.InterestAmountInvalid" });
    expect(mockRecordInterest).not.toHaveBeenCalled();
  });

  // INT-021 — percent bounds surface as an inline error on submit; the gate
  // stays open so the message is reachable.
  it("surfaces a percent-bounds error on submit instead of disabling the gate", async () => {
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));
    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
      result.current.handleChange("percent", "0");
    });
    expect(result.current.isFormValid).toBe(true);
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(result.current.error).not.toBeNull();
    expect(mockRecordInterest).not.toHaveBeenCalled();
  });

  // INT-021 — zero quantity surfaces as an inline error on submit.
  it("surfaces a zero-quantity error on submit instead of disabling the gate", async () => {
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));
    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
      result.current.handleChange("quantity", "0");
    });
    expect(result.current.isFormValid).toBe(true);
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(result.current.error).not.toBeNull();
    expect(mockRecordInterest).not.toHaveBeenCalled();
  });

  // INT-021 — form valid with asset + percent only
  it("isFormValid true when asset selected, percent positive, and quantity blank", () => {
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));
    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
      result.current.handleChange("percent", "2.5");
    });
    expect(result.current.isFormValid).toBe(true);
  });

  // INT-021 — form valid with asset + quantity only
  it("isFormValid true when asset selected, quantity positive, and percent blank", () => {
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));
    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
      result.current.handleChange("quantity", "10");
    });
    expect(result.current.isFormValid).toBe(true);
  });

  // INT-020 — no amount/exchange-rate/fees fields (these do not exist on the form type)
  it("formData has no amount, exchangeRate, or fees fields (INT-020 — no money inputs)", () => {
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));
    expect("amount" in result.current.formData).toBe(false);
    expect("exchangeRate" in result.current.formData).toBe(false);
    expect("fees" in result.current.formData).toBe(false);
  });

  // INT-021/022 — percent mode submits percent_micros and a null quantity_micros
  it("submits percent mode with percent in micro-percent and quantity_micros null", async () => {
    mockRecordInterest.mockResolvedValue({ status: "ok", data: { id: "tx-int-1" } });
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
      result.current.handleChange("percent", "2.5");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordInterest).toHaveBeenCalledWith({
      account_id: "account-1",
      asset_id: "asset-fund-1",
      date: TODAY,
      percent_micros: 2_500_000,
      quantity_micros: null,
      note: null,
    });
  });

  // INT-021 — quantity mode submits quantity_micros and a null percent_micros
  it("submits quantity mode with quantity in micros and percent_micros null", async () => {
    mockRecordInterest.mockResolvedValue({ status: "ok", data: { id: "tx-int-1" } });
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
      result.current.handleChange("quantity", "5.5");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordInterest).toHaveBeenCalledWith(
      expect.objectContaining({
        account_id: "account-1",
        asset_id: "asset-fund-1",
        percent_micros: null,
        quantity_micros: 5_500_000,
        note: null,
      }),
    );
  });

  // INT-021 — both filled: inline InterestAmountInvalid error, no gateway call
  it("sets error.InterestAmountInvalid and skips the gateway when both fields are filled", async () => {
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
      result.current.handleChange("percent", "2");
      result.current.handleChange("quantity", "10");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.InterestAmountInvalid" });
    expect(mockRecordInterest).not.toHaveBeenCalled();
  });

  // INT-021 — neither filled: same InterestAmountInvalid error
  it("sets error.InterestAmountInvalid when neither percent nor quantity is filled", async () => {
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.InterestAmountInvalid" });
    expect(mockRecordInterest).not.toHaveBeenCalled();
  });

  // INT-025 — success: snackbar shown, onSubmitSuccess called
  it("shows success snackbar and calls onSubmitSuccess on ok result", async () => {
    mockRecordInterest.mockResolvedValue({ status: "ok", data: { id: "tx-int-1" } });
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
      result.current.handleChange("quantity", "10");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("interest.recorded", "success");
    expect(BASE_PROPS.onSubmitSuccess).toHaveBeenCalled();
  });

  // INT-011 — error result sets inline error via presenter (F27 no-throw)
  it("surfaces backend error code as inline error on error result (F27)", async () => {
    mockRecordInterest.mockResolvedValue({
      status: "error",
      error: { code: "AssetNotHeld" },
    });
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
      result.current.handleChange("quantity", "10");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.AssetNotHeld" });
    expect(mockShowSnackbar).not.toHaveBeenCalled();
  });

  // DatabaseError — logged and mapped to i18n key
  it("logs and maps DatabaseError to inline i18n key", async () => {
    mockRecordInterest.mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
      result.current.handleChange("quantity", "10");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(logger.error).toHaveBeenCalledWith(
      "[useInterestTransaction] recordInterest failed",
      expect.objectContaining({ error: { code: "DatabaseError" } }),
    );
    expect(mockShowSnackbar).not.toHaveBeenCalled();
  });

  // INT-025 — isSubmitting toggles during the request
  it("isSubmitting is true during submit and false after", async () => {
    let resolvePromise!: (value: unknown) => void;
    mockRecordInterest.mockReturnValue(
      new Promise((resolve) => {
        resolvePromise = resolve;
      }),
    );
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-fund-1");
      result.current.handleChange("quantity", "10");
    });

    act(() => {
      void result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.isSubmitting).toBe(true);

    await act(async () => {
      resolvePromise({ status: "ok", data: { id: "tx-int-1" } });
    });

    expect(result.current.isSubmitting).toBe(false);
  });

  // INT-021 — submit blocked when form is invalid (no gateway call)
  it("handleSubmit does not call gateway when form is invalid", async () => {
    const { result } = renderHook(() => useInterestTransaction(BASE_PROPS));
    // No assetId, no percent/quantity — form is invalid

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordInterest).not.toHaveBeenCalled();
  });

  // INT-040 — edit mode: correctTransaction called with date/quantity/note (asset locked)
  it("in edit mode, submit calls correctTransaction with the zero-cost packing and asset is immutable", async () => {
    mockCorrectTransaction.mockResolvedValue({ status: "ok", data: {} });

    const editProps = {
      ...BASE_PROPS,
      editMode: {
        transactionId: "tx-int-existing",
        lockedAssetId: "asset-fund-1",
        lockedAssetName: "Euro Fund",
      },
    };

    const { result } = renderHook(() => useInterestTransaction(editProps));

    // The asset selector is locked (not changeable) in edit mode.
    expect(result.current.isAssetLocked).toBe(true);

    act(() => {
      result.current.handleChange("quantity", "3");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    // INT-040 — edit reuses correct_transaction with the corrected quantity (micros),
    // the zero-cost convention on the money fields, and the original transaction id.
    expect(mockCorrectTransaction).toHaveBeenCalledWith("tx-int-existing", "account-1", {
      date: TODAY,
      quantity: 3_000_000,
      unit_price: 0,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: null,
      note: null,
    });
    // The create path must NOT be used in edit mode.
    expect(mockRecordInterest).not.toHaveBeenCalled();
    expect(mockShowSnackbar).toHaveBeenCalledWith("interest.updated", "success");
  });
});
