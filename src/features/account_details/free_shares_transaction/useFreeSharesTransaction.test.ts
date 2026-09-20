import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { logger } from "@/lib/logger";
import { useFreeSharesTransaction } from "./useFreeSharesTransaction";

// ── Hoisted mocks ──────────────────────────────────────────────────────────────
const { mockRecordFreeShares, mockCorrectTransaction, mockShowSnackbar } = vi.hoisted(() => ({
  mockRecordFreeShares: vi.fn(),
  mockCorrectTransaction: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    recordFreeShares: mockRecordFreeShares,
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

const heldAssets = [
  { assetId: "asset-equity-1", assetName: "Apple Inc", assetCurrency: "EUR" },
  { assetId: "asset-equity-2", assetName: "Tesla Inc", assetCurrency: "USD" },
];

const BASE_PROPS = {
  accountId: "account-1",
  heldAssets,
  onSubmitSuccess: vi.fn(),
};

// ── Tests ──────────────────────────────────────────────────────────────────────
describe("useFreeSharesTransaction (FSD-020/021/025)", () => {
  beforeEach(() => {
    mockRecordFreeShares.mockReset();
    mockCorrectTransaction.mockReset();
    mockShowSnackbar.mockReset();
    vi.mocked(logger.error).mockClear();
    BASE_PROPS.onSubmitSuccess.mockClear();
  });

  // FSD-020 — initial form has today's date, no asset selected, blank quantity, blank note
  it("initial state has today's date, no asset selected, blank quantity, and blank note", () => {
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));
    expect(result.current.formData.date).toBe(TODAY);
    expect(result.current.formData.assetId).toBe("");
    expect(result.current.formData.quantity).toBe("");
    expect(result.current.formData.note).toBe("");
  });

  // FSD-021 — form invalid when no asset selected
  it("isFormValid false when no asset is selected", () => {
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));
    act(() => result.current.handleChange("quantity", "10"));
    expect(result.current.isFormValid).toBe(false);
  });

  // FSD-021 — form invalid when quantity is blank
  it("isFormValid false when asset selected but quantity is blank", () => {
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));
    act(() => result.current.handleChange("assetId", "asset-equity-1"));
    expect(result.current.isFormValid).toBe(false);
  });

  // FSD-021 — form invalid when quantity is zero
  it("isFormValid false when quantity is zero", () => {
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));
    act(() => {
      result.current.handleChange("assetId", "asset-equity-1");
      result.current.handleChange("quantity", "0");
    });
    expect(result.current.isFormValid).toBe(false);
  });

  // FSD-021 — form invalid when quantity is negative
  it("isFormValid false when quantity is negative", () => {
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));
    act(() => {
      result.current.handleChange("assetId", "asset-equity-1");
      result.current.handleChange("quantity", "-5");
    });
    expect(result.current.isFormValid).toBe(false);
  });

  // FSD-021 — form valid with asset + positive quantity + valid date
  it("isFormValid true when asset selected, quantity positive, and date valid", () => {
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));
    act(() => {
      result.current.handleChange("assetId", "asset-equity-1");
      result.current.handleChange("quantity", "5");
    });
    expect(result.current.isFormValid).toBe(true);
  });

  // FSD-020 — no amount/exchange-rate/fees fields (these do not exist on the form type)
  it("formData has no amount, exchangeRate, or fees fields (FSD-020 — no money inputs)", () => {
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));
    expect("amount" in result.current.formData).toBe(false);
    expect("exchangeRate" in result.current.formData).toBe(false);
    expect("fees" in result.current.formData).toBe(false);
  });

  // FSD-022 — valid submit calls gateway with micro-converted quantity and no money fields
  it("submits and calls gateway with quantity in micros, no amount/exchange_rate/fees", async () => {
    mockRecordFreeShares.mockResolvedValue({ status: "ok", data: { id: "tx-fsd-1" } });
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-equity-1");
      result.current.handleChange("quantity", "5.5");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordFreeShares).toHaveBeenCalledWith(
      expect.objectContaining({
        account_id: "account-1",
        asset_id: "asset-equity-1",
        quantity: 5_500_000,
        note: null,
      }),
    );
    // Must NOT contain amount, exchange_rate, or fees
    const callArgs = mockRecordFreeShares.mock.calls[0]?.[0] as Record<string, unknown>;
    expect("amount_micros" in callArgs).toBe(false);
    expect("exchange_rate" in callArgs).toBe(false);
    expect("fees" in callArgs).toBe(false);
  });

  // FSD-025 — success: snackbar shown, onSubmitSuccess called
  it("shows success snackbar and calls onSubmitSuccess on ok result", async () => {
    mockRecordFreeShares.mockResolvedValue({ status: "ok", data: { id: "tx-fsd-1" } });
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-equity-1");
      result.current.handleChange("quantity", "10");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("free_shares.recorded", "success");
    expect(BASE_PROPS.onSubmitSuccess).toHaveBeenCalled();
  });

  // FSD-025 — error result sets inline error via presenter (F27 no-throw)
  it("surfaces backend error code as inline error on error result (F27)", async () => {
    mockRecordFreeShares.mockResolvedValue({
      status: "error",
      error: { code: "AssetNotHeld" },
    });
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-equity-1");
      result.current.handleChange("quantity", "10");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.AssetNotHeld" });
    expect(mockShowSnackbar).not.toHaveBeenCalled();
  });

  // FSD-011 — FreeSharesOnCashAsset error maps to i18n key
  it("maps FreeSharesOnCashAsset error to its i18n key", async () => {
    mockRecordFreeShares.mockResolvedValue({
      status: "error",
      error: { code: "FreeSharesOnCashAsset" },
    });
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-equity-1");
      result.current.handleChange("quantity", "10");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.FreeSharesOnCashAsset" });
  });

  // DatabaseError — logged and mapped to i18n key
  it("logs and maps DatabaseError to inline i18n key", async () => {
    mockRecordFreeShares.mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-equity-1");
      result.current.handleChange("quantity", "10");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(logger.error).toHaveBeenCalledWith(
      "[useFreeSharesTransaction] recordFreeShares failed",
      expect.objectContaining({ error: { code: "DatabaseError" } }),
    );
    expect(mockShowSnackbar).not.toHaveBeenCalled();
  });

  // FSD-025 — isSubmitting toggles during the request
  it("isSubmitting is true during submit and false after", async () => {
    let resolvePromise!: (value: unknown) => void;
    mockRecordFreeShares.mockReturnValue(
      new Promise((resolve) => {
        resolvePromise = resolve;
      }),
    );
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-equity-1");
      result.current.handleChange("quantity", "10");
    });

    act(() => {
      void result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.isSubmitting).toBe(true);

    await act(async () => {
      resolvePromise({ status: "ok", data: { id: "tx-fsd-1" } });
    });

    expect(result.current.isSubmitting).toBe(false);
  });

  // FSD-021 — submit blocked when form is invalid (no gateway call)
  it("handleSubmit does not call gateway when form is invalid", async () => {
    const { result } = renderHook(() => useFreeSharesTransaction(BASE_PROPS));
    // No assetId, no quantity — form is invalid

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordFreeShares).not.toHaveBeenCalled();
  });

  // FSD-040 — edit mode: correctTransaction called with date/quantity/note (asset locked)
  it("in edit mode, submit calls correctTransaction with date/quantity/note and asset is immutable", async () => {
    mockCorrectTransaction.mockResolvedValue({ status: "ok", data: {} });

    const editProps = {
      ...BASE_PROPS,
      editMode: {
        transactionId: "tx-fsd-existing",
        lockedAssetId: "asset-equity-1",
        lockedAssetName: "Apple Inc",
      },
    };

    const { result } = renderHook(() => useFreeSharesTransaction(editProps));

    // The asset selector is locked (not changeable) in edit mode.
    expect(result.current.isAssetLocked).toBe(true);

    act(() => {
      result.current.handleChange("quantity", "3");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    // FSD-040 — edit reuses correct_transaction with the corrected quantity (micros),
    // the zero-cost convention on the money fields, and the original transaction id.
    expect(mockCorrectTransaction).toHaveBeenCalledWith(
      "tx-fsd-existing",
      "account-1",
      expect.objectContaining({ quantity: 3_000_000, unit_price: 0, fees: 0 }),
    );
    // The create path must NOT be used in edit mode.
    expect(mockRecordFreeShares).not.toHaveBeenCalled();
  });
});
