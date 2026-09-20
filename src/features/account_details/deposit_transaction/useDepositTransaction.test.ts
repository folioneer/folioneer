import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { logger } from "@/lib/logger";
import { useDepositTransaction } from "./useDepositTransaction";

const { mockRecordDeposit, mockCorrectTransaction, mockShowSnackbar } = vi.hoisted(() => ({
  mockRecordDeposit: vi.fn(),
  mockCorrectTransaction: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    recordDeposit: mockRecordDeposit,
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

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

describe("useDepositTransaction (CSH-020/021/022/025)", () => {
  beforeEach(() => {
    mockRecordDeposit.mockReset();
    mockCorrectTransaction.mockReset();
    mockShowSnackbar.mockReset();
    vi.mocked(logger.error).mockClear();
  });

  // CSH-020 — initial form has today's date and empty amount
  it("initial state has today's date and blank amount/note", () => {
    const { result } = renderHook(() => useDepositTransaction({ accountId: "account-1" }));
    expect(result.current.formData.date).toBe(new Date().toISOString().slice(0, 10));
    expect(result.current.formData.amount).toBe("");
    expect(result.current.formData.note).toBe("");
  });

  // CSH-021 — empty amount makes the form invalid
  it("isFormValid false when amount is blank", () => {
    const { result } = renderHook(() => useDepositTransaction({ accountId: "account-1" }));
    expect(result.current.isFormValid).toBe(false);
  });

  // CSH-021 — amount > 0 makes the form valid
  it("isFormValid true when amount is positive and date is valid", () => {
    const { result } = renderHook(() => useDepositTransaction({ accountId: "account-1" }));
    act(() => result.current.handleChange("amount", "100"));
    expect(result.current.isFormValid).toBe(true);
  });

  // CSH-022 / CSH-025 — successful submit calls gateway then shows success snackbar
  it("submits and fires success snackbar on success", async () => {
    mockRecordDeposit.mockResolvedValue({ status: "ok", data: { id: "tx-1" } });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() =>
      useDepositTransaction({ accountId: "account-1", onSubmitSuccess }),
    );

    act(() => result.current.handleChange("amount", "250.50"));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordDeposit).toHaveBeenCalledWith(
      expect.objectContaining({
        account_id: "account-1",
        amount_micros: 250_500_000,
      }),
    );
    expect(mockShowSnackbar).toHaveBeenCalledWith("cash.deposit_recorded", "success");
    expect(onSubmitSuccess).toHaveBeenCalled();
  });

  // CSH-021 — backend rejects AmountNotPositive → inline error key set
  it("surfaces backend error code as inline error", async () => {
    mockRecordDeposit.mockResolvedValue({
      status: "error",
      error: { code: "AmountNotPositive" },
    });
    const { result } = renderHook(() => useDepositTransaction({ accountId: "account-1" }));

    act(() => result.current.handleChange("amount", "100"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.AmountNotPositive" });
    expect(mockShowSnackbar).not.toHaveBeenCalled();
  });

  // DatabaseError path — the presenter maps to error.DatabaseError; logger keeps
  // the full payload server-side via tracing for triage outside the user-visible message.
  it("logs full error and maps DatabaseError to inline i18n key", async () => {
    mockRecordDeposit.mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const { result } = renderHook(() => useDepositTransaction({ accountId: "account-1" }));

    act(() => result.current.handleChange("amount", "100"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(logger.error).toHaveBeenCalledWith("[useDepositTransaction] submit failed", {
      error: { code: "DatabaseError" },
    });
    expect(mockShowSnackbar).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// CSH-111 — edit mode: prefill from the existing Deposit + persist via
// correct_transaction (not record_deposit), with the "updated" snackbar.
// ---------------------------------------------------------------------------
const editDeposit = {
  id: "tx-dep-1",
  account_id: "account-1",
  asset_id: "system-cash-eur",
  transaction_type: "Deposit",
  date: "2026-05-01",
  quantity: 500_000_000,
  unit_price: 1_000_000,
  exchange_rate: 1_000_000,
  fees: 0,
  total_amount: 500_000_000,
  note: "rent",
  realized_pnl: null,
  created_at: "2026-05-01T00:00:00Z",
} as const;

describe("useDepositTransaction — edit mode (CSH-111)", () => {
  beforeEach(() => {
    mockRecordDeposit.mockReset();
    mockCorrectTransaction.mockReset();
    mockShowSnackbar.mockReset();
  });

  it("prefills the form from the edited Deposit (date, amount, note)", () => {
    const { result } = renderHook(() =>
      useDepositTransaction({ accountId: "account-1", editTransaction: editDeposit }),
    );
    expect(result.current.formData.date).toBe("2026-05-01");
    expect(result.current.formData.amount).toBe("500.000");
    expect(result.current.formData.note).toBe("rent");
  });

  it("submits via correctTransaction (not recordDeposit) and shows the updated snackbar", async () => {
    mockCorrectTransaction.mockResolvedValue({ status: "ok", data: { id: "tx-dep-1" } });
    const { result } = renderHook(() =>
      useDepositTransaction({ accountId: "account-1", editTransaction: editDeposit }),
    );

    act(() => result.current.handleChange("amount", "650"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordDeposit).not.toHaveBeenCalled();
    expect(mockCorrectTransaction).toHaveBeenCalledWith(
      "tx-dep-1",
      "account-1",
      expect.objectContaining({ date: "2026-05-01", quantity: 650_000_000, note: "rent" }),
    );
    expect(mockShowSnackbar).toHaveBeenCalledWith("cash.deposit_updated", "success");
  });
});

describe("useDepositTransaction — edit mode error path (CSH-111)", () => {
  beforeEach(() => {
    mockRecordDeposit.mockReset();
    mockCorrectTransaction.mockReset();
    mockShowSnackbar.mockReset();
    vi.mocked(logger.error).mockClear();
  });

  it("surfaces the backend error inline and does not snackbar when correctTransaction fails", async () => {
    mockCorrectTransaction.mockResolvedValue({
      status: "error",
      error: { code: "InsufficientCash", current_balance_micros: 10_000_000, currency: "EUR" },
    });
    const { result } = renderHook(() =>
      useDepositTransaction({ accountId: "account-1", editTransaction: editDeposit }),
    );

    act(() => result.current.handleChange("amount", "999"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordDeposit).not.toHaveBeenCalled();
    expect(result.current.error).toEqual({
      key: "cash.insufficient_cash_inline",
      vars: { balance: "10,00", currency: "EUR" },
    });
    expect(mockShowSnackbar).not.toHaveBeenCalled();
    expect(logger.error).toHaveBeenCalledWith(
      "[useDepositTransaction] submit failed",
      expect.objectContaining({ error: expect.objectContaining({ code: "InsufficientCash" }) }),
    );
  });

  it("falls back to the Unknown error when the gateway throws", async () => {
    mockCorrectTransaction.mockRejectedValue(new Error("ipc down"));
    const { result } = renderHook(() =>
      useDepositTransaction({ accountId: "account-1", editTransaction: editDeposit }),
    );

    act(() => result.current.handleChange("amount", "10"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.Unknown" });
  });
});
