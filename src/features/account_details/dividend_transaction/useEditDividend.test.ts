import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { microToFormatted } from "@/lib/microUnits";
import { type EditedDividend, useEditDividend } from "./useEditDividend";

const { mockValidateDraft, mockCorrectTransaction, mockShowSnackbar } = vi.hoisted(() => ({
  mockValidateDraft: vi.fn(),
  mockCorrectTransaction: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    validateTransactionDraft: mockValidateDraft,
    correctTransaction: mockCorrectTransaction,
  },
}));
vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));
vi.mock("@/lib/logger", () => ({ logger: { error: vi.fn(), info: vi.fn() } }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const dividend: EditedDividend = {
  transactionId: "tx-div-1",
  accountId: "account-1",
  assetId: "asset-1",
  date: "2026-09-15",
  amount: "18.6",
  exchangeRate: "0.9214",
  note: "Interim",
  unitPriceMicro: 1_000_000,
};
const submit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

describe("useEditDividend (DIV-040)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockValidateDraft.mockResolvedValue({
      status: "ok",
      data: { unit_price: 1_000_000, total_amount: 17_138_040 },
    });
    mockCorrectTransaction.mockResolvedValue({ status: "ok", data: null });
  });

  // TRX-062 — the dialog sends a dividend draft and shows the total the core returns;
  // it holds no formula of its own.
  it("checks the dividend with the core and shows the total it returns", async () => {
    const { result } = renderHook(() => useEditDividend({ dividend, onSubmitSuccess: vi.fn() }));

    await waitFor(() => expect(result.current.isFormValid).toBe(true));

    expect(mockValidateDraft).toHaveBeenLastCalledWith({
      kind: "Dividend",
      account_id: "account-1",
      asset_id: "asset-1",
      date: "2026-09-15",
      quantity: 18_600_000,
      entered: { mode: "UnitPrice", unit_price: 1_000_000, exchange_rate: 921_400, fees: 0 },
      correcting: "tx-div-1",
    });
    expect(result.current.totalDisplay).toBe(microToFormatted(17_138_040));
  });

  // DIV-040 — the correction carries the date, the amount, the rate and the note; the
  // stored unit price goes back unchanged, with no fees and no typed total.
  it("records the correction and reports it", async () => {
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useEditDividend({ dividend, onSubmitSuccess }));
    await waitFor(() => expect(result.current.isFormValid).toBe(true));

    act(() => result.current.handleChange("amount", "20"));
    act(() => result.current.handleChange("note", ""));
    await waitFor(() => expect(result.current.isFormValid).toBe(true));
    await act(async () => result.current.handleSubmit(submit));

    expect(mockCorrectTransaction).toHaveBeenCalledWith("tx-div-1", "account-1", {
      date: "2026-09-15",
      quantity: 20_000_000,
      unit_price: 1_000_000,
      exchange_rate: 921_400,
      fees: 0,
      total_amount: null,
      note: null,
    });
    expect(mockShowSnackbar).toHaveBeenCalledWith("dividend.updated", "success");
    expect(onSubmitSuccess).toHaveBeenCalledTimes(1);
  });

  // DIV-044 — saved untouched, a dividend goes back as it was recorded: its stored unit
  // price and rate, no fees, nothing a trade would carry.
  it("saves an untouched same-currency dividend exactly as recorded", async () => {
    const same = { ...dividend, amount: "12.5", exchangeRate: "1", note: "" };
    const { result } = renderHook(() =>
      useEditDividend({ dividend: same, onSubmitSuccess: vi.fn() }),
    );
    await waitFor(() => expect(result.current.isFormValid).toBe(true));

    await act(async () => result.current.handleSubmit(submit));

    expect(mockCorrectTransaction).toHaveBeenCalledWith("tx-div-1", "account-1", {
      date: "2026-09-15",
      quantity: 12_500_000,
      unit_price: 1_000_000,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: null,
      note: null,
    });
  });

  // A parent that re-renders with the same dividend does not start the check again.
  it("does not check again when re-rendered with the same dividend", async () => {
    const { result, rerender } = renderHook(
      ({ value }) => useEditDividend({ dividend: value, onSubmitSuccess: vi.fn() }),
      { initialProps: { value: dividend } },
    );
    await waitFor(() => expect(result.current.isFormValid).toBe(true));
    const checks = mockValidateDraft.mock.calls.length;

    rerender({ value: { ...dividend } });

    expect(result.current.isFormValid).toBe(true);
    expect(mockValidateDraft.mock.calls.length).toBe(checks);
  });

  // TRX-067 — the core's first problem shows on the field it concerns once that field was
  // typed in, and saving is not possible meanwhile.
  it("shows the core's problem on the amount once it was typed in", async () => {
    const { result } = renderHook(() => useEditDividend({ dividend, onSubmitSuccess: vi.fn() }));
    await waitFor(() => expect(result.current.isFormValid).toBe(true));

    mockValidateDraft.mockResolvedValue({
      status: "error",
      error: { code: "QuantityNotPositive" },
    });
    act(() => result.current.handleChange("amount", "0"));

    await waitFor(() => expect(result.current.fieldErrors.quantity).toBeDefined());
    expect(result.current.isFormValid).toBe(false);
    expect(result.current.totalDisplay).toBeNull();

    await act(async () => result.current.handleSubmit(submit));
    expect(mockCorrectTransaction).not.toHaveBeenCalled();
  });

  // F27 — a correction the core refuses on save (cash would run short later) is shown and
  // the dialog stays open.
  it("shows a refusal on save and does not close", async () => {
    mockCorrectTransaction.mockResolvedValue({
      status: "error",
      error: { code: "InsufficientCash", current_balance_micros: 0, currency: "EUR" },
    });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useEditDividend({ dividend, onSubmitSuccess }));
    await waitFor(() => expect(result.current.isFormValid).toBe(true));

    await act(async () => result.current.handleSubmit(submit));

    expect(result.current.error).not.toBeNull();
    expect(onSubmitSuccess).not.toHaveBeenCalled();
    expect(result.current.isSubmitting).toBe(false);
  });
});
