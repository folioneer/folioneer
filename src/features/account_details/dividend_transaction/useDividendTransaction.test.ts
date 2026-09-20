import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { logger } from "@/lib/logger";
import { useDividendTransaction } from "./useDividendTransaction";

const { mockRecordDividend, mockShowSnackbar } = vi.hoisted(() => ({
  mockRecordDividend: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    recordDividend: mockRecordDividend,
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

const TODAY = new Date().toISOString().slice(0, 10);

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

// Shared held assets for tests
const heldAssets = [
  { assetId: "asset-eur-1", assetName: "Apple Inc", assetCurrency: "EUR" },
  { assetId: "asset-usd-1", assetName: "Tesla Inc", assetCurrency: "USD" },
];

const BASE_PROPS = {
  accountId: "account-1",
  accountCurrency: "EUR",
  heldAssets,
  onSubmitSuccess: vi.fn(),
};

describe("useDividendTransaction (DIV-020/021/022/025)", () => {
  beforeEach(() => {
    mockRecordDividend.mockReset();
    mockShowSnackbar.mockReset();
    vi.mocked(logger.error).mockClear();
    BASE_PROPS.onSubmitSuccess.mockClear();
  });

  // DIV-020 — initial form has today's date, empty asset, empty amount, rate "1.000000"
  it("initial state has today's date, no asset selected, blank amount, and default exchange rate", () => {
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));
    expect(result.current.formData.date).toBe(TODAY);
    expect(result.current.formData.assetId).toBe("");
    expect(result.current.formData.amount).toBe("");
    expect(result.current.formData.exchangeRate).toBe("1.000000");
    expect(result.current.formData.note).toBe("");
  });

  // DIV-021 — form invalid when no asset selected
  it("isFormValid false when no asset is selected", () => {
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));
    act(() => result.current.handleChange("amount", "100"));
    expect(result.current.isFormValid).toBe(false);
  });

  // DIV-021 — form invalid when amount is blank
  it("isFormValid false when asset selected but amount is blank", () => {
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));
    act(() => result.current.handleChange("assetId", "asset-eur-1"));
    expect(result.current.isFormValid).toBe(false);
  });

  // DIV-021 — form valid with asset + positive amount + valid date
  it("isFormValid true when asset selected, amount positive, and date valid", () => {
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));
    act(() => {
      result.current.handleChange("assetId", "asset-eur-1");
      result.current.handleChange("amount", "50.00");
    });
    expect(result.current.isFormValid).toBe(true);
  });

  // DIV-022 — showExchangeRate false when asset currency matches account currency
  it("showExchangeRate is false when selected asset currency matches account currency", () => {
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));
    act(() => result.current.handleChange("assetId", "asset-eur-1")); // EUR == EUR
    expect(result.current.showExchangeRate).toBe(false);
  });

  // DIV-022 — showExchangeRate true when asset currency differs from account currency
  it("showExchangeRate is true when selected asset currency differs from account currency", () => {
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));
    act(() => result.current.handleChange("assetId", "asset-usd-1")); // USD != EUR
    expect(result.current.showExchangeRate).toBe(true);
  });

  // DIV-023 — valid submit calls gateway with correct micro-unit conversion
  it("submits and calls gateway with amount_micros and exchange_rate in micros", async () => {
    mockRecordDividend.mockResolvedValue({ status: "ok", data: { id: "tx-div-1" } });
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-eur-1");
      result.current.handleChange("amount", "75.50");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordDividend).toHaveBeenCalledWith(
      expect.objectContaining({
        account_id: "account-1",
        asset_id: "asset-eur-1",
        amount_micros: 75_500_000,
        exchange_rate: 1_000_000,
        note: null,
      }),
    );
  });

  // DIV-025 — success: snackbar shown, onSubmitSuccess called
  it("shows success snackbar and calls onSubmitSuccess on ok result", async () => {
    mockRecordDividend.mockResolvedValue({ status: "ok", data: { id: "tx-div-1" } });
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-eur-1");
      result.current.handleChange("amount", "100");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("dividend.recorded", "success");
    expect(BASE_PROPS.onSubmitSuccess).toHaveBeenCalled();
  });

  // DIV-010 — "add another" records, refreshes via onRecorded (not onSubmitSuccess),
  // and clears amount + note while keeping the asset + date for the next entry.
  it("handleAddAnother records, calls onRecorded, and clears amount + note", async () => {
    mockRecordDividend.mockResolvedValue({ status: "ok", data: { id: "tx-div-1" } });
    const onRecorded = vi.fn();
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() =>
      useDividendTransaction({ ...BASE_PROPS, onRecorded, onSubmitSuccess }),
    );

    act(() => {
      result.current.handleChange("assetId", "asset-eur-1");
      result.current.handleChange("amount", "100");
      result.current.handleChange("note", "Q1 payout");
    });

    await act(async () => {
      await result.current.handleAddAnother();
    });

    expect(onRecorded).toHaveBeenCalledTimes(1);
    expect(onSubmitSuccess).not.toHaveBeenCalled();
    expect(result.current.formData.amount).toBe("");
    expect(result.current.formData.note).toBe("");
    expect(result.current.formData.assetId).toBe("asset-eur-1"); // kept for repeat entry
  });

  // DIV-025 — error result sets inline error via presenter
  it("surfaces backend error code as inline error on error result", async () => {
    mockRecordDividend.mockResolvedValue({
      status: "error",
      error: { code: "AssetNotHeld" },
    });
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-eur-1");
      result.current.handleChange("amount", "100");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.AssetNotHeld" });
    expect(mockShowSnackbar).not.toHaveBeenCalled();
  });

  // DatabaseError — logged and mapped to i18n key
  it("logs and maps DatabaseError to inline i18n key", async () => {
    mockRecordDividend.mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-eur-1");
      result.current.handleChange("amount", "100");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(logger.error).toHaveBeenCalledWith(
      "[useDividendTransaction] recordDividend failed",
      expect.objectContaining({ error: { code: "DatabaseError" } }),
    );
    expect(mockShowSnackbar).not.toHaveBeenCalled();
  });

  // DIV-025 — isSubmitting toggles during the request
  it("isSubmitting is true during submit and false after", async () => {
    let resolvePromise!: (value: unknown) => void;
    mockRecordDividend.mockReturnValue(
      new Promise((resolve) => {
        resolvePromise = resolve;
      }),
    );
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-eur-1");
      result.current.handleChange("amount", "100");
    });

    act(() => {
      void result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.isSubmitting).toBe(true);

    await act(async () => {
      resolvePromise({ status: "ok", data: { id: "tx-div-1" } });
    });

    expect(result.current.isSubmitting).toBe(false);
  });

  // DIV-021 — submit blocked when form is invalid (no gateway call)
  it("handleSubmit does not call gateway when form is invalid", async () => {
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));
    // No assetId, no amount — form is invalid

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordDividend).not.toHaveBeenCalled();
  });

  // DIV-022 — foreign-currency asset submit sends the user-supplied exchange rate in micros
  it("submits exchange_rate in micros when asset currency differs from account currency", async () => {
    mockRecordDividend.mockResolvedValue({ status: "ok", data: { id: "tx-div-2" } });
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));

    act(() => {
      result.current.handleChange("assetId", "asset-usd-1"); // USD != EUR, showExchangeRate true
      result.current.handleChange("amount", "50");
      result.current.handleChange("exchangeRate", "1.08");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordDividend).toHaveBeenCalledWith(
      expect.objectContaining({
        asset_id: "asset-usd-1",
        amount_micros: 50_000_000,
        exchange_rate: 1_080_000,
      }),
    );
  });

  // DIV-028 — the entry-mode switch exists only for a foreign-currency asset,
  // and switching to account-currency mode hides the exchange rate.
  it("offers the account-currency mode only for a foreign asset and hides the rate in it", () => {
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));

    act(() => result.current.handleChange("assetId", "asset-eur-1"));
    expect(result.current.showCurrencyModeSwitch).toBe(false);

    act(() => result.current.handleChange("assetId", "asset-usd-1"));
    expect(result.current.showCurrencyModeSwitch).toBe(true);
    expect(result.current.showExchangeRate).toBe(true);
    expect(result.current.amountCurrency).toBe("USD");

    act(() => result.current.setAmountInAccountCurrency(true));
    expect(result.current.showExchangeRate).toBe(false);
    expect(result.current.amountCurrency).toBe("EUR");
  });

  // DIV-029 — account-currency mode records the typed amount verbatim with rate 1,
  // ignoring whatever rate the form previously held.
  it("records the typed amount with an exchange rate of 1 in account-currency mode", async () => {
    mockRecordDividend.mockResolvedValue({ status: "ok", data: {} });
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));

    act(() => result.current.handleChange("assetId", "asset-usd-1"));
    act(() => result.current.handleChange("amount", "250"));
    act(() => result.current.handleChange("exchangeRate", "0.92"));
    act(() => result.current.setAmountInAccountCurrency(true));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordDividend).toHaveBeenCalledWith(
      expect.objectContaining({
        asset_id: "asset-usd-1",
        amount_micros: 250_000_000,
        exchange_rate: 1_000_000,
      }),
    );
  });

  // DIV-022 unchanged — asset-currency mode still converts via the supplied rate.
  it("keeps the supplied rate when the amount stays in the asset currency", async () => {
    mockRecordDividend.mockResolvedValue({ status: "ok", data: {} });
    const { result } = renderHook(() => useDividendTransaction(BASE_PROPS));

    act(() => result.current.handleChange("assetId", "asset-usd-1"));
    act(() => result.current.handleChange("amount", "250"));
    act(() => result.current.handleChange("exchangeRate", "0.92"));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordDividend).toHaveBeenCalledWith(
      expect.objectContaining({ exchange_rate: 920_000 }),
    );
  });
});
