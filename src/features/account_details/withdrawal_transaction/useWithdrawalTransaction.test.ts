import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { logger } from "@/lib/logger";
import { useWithdrawalTransaction } from "./useWithdrawalTransaction";

const { mockRecordWithdrawal, mockCorrectTransaction, mockShowSnackbar } = vi.hoisted(() => ({
  mockRecordWithdrawal: vi.fn(),
  mockCorrectTransaction: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    recordWithdrawal: mockRecordWithdrawal,
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
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}::${JSON.stringify(opts)}` : key,
    i18n: { language: "en" },
  }),
}));

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

describe("useWithdrawalTransaction (CSH-030/031/032/035/081)", () => {
  beforeEach(() => {
    mockRecordWithdrawal.mockReset();
    mockShowSnackbar.mockReset();
    vi.mocked(logger.error).mockClear();
  });

  // CSH-030 — initial state
  it("initial state has today's date and blank amount/note", () => {
    const { result } = renderHook(() => useWithdrawalTransaction({ accountId: "account-1" }));
    expect(result.current.formData.date).toBe(new Date().toISOString().slice(0, 10));
    expect(result.current.formData.amount).toBe("");
  });

  // CSH-031 — empty amount → invalid
  it("isFormValid false when amount blank", () => {
    const { result } = renderHook(() => useWithdrawalTransaction({ accountId: "account-1" }));
    expect(result.current.isFormValid).toBe(false);
  });

  // CSH-032 / CSH-035 — happy path
  it("submits and fires success snackbar on success", async () => {
    mockRecordWithdrawal.mockResolvedValue({ status: "ok", data: { id: "tx-1" } });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() =>
      useWithdrawalTransaction({ accountId: "account-1", onSubmitSuccess }),
    );

    act(() => result.current.handleChange("amount", "75.25"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordWithdrawal).toHaveBeenCalledWith(
      expect.objectContaining({
        account_id: "account-1",
        amount_micros: 75_250_000,
      }),
    );
    expect(mockShowSnackbar).toHaveBeenCalledWith("cash.withdrawal_recorded", "success");
    expect(onSubmitSuccess).toHaveBeenCalled();
  });

  // CSH-081 — InsufficientCash includes balance + currency in inline error
  it("renders InsufficientCash inline error with balance + currency interpolation", async () => {
    mockRecordWithdrawal.mockResolvedValue({
      status: "error",
      error: {
        code: "InsufficientCash",
        current_balance_micros: 50_000_000,
        currency: "EUR",
      },
    });
    const { result } = renderHook(() => useWithdrawalTransaction({ accountId: "account-1" }));

    act(() => result.current.handleChange("amount", "999"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({
      key: "cash.insufficient_cash_inline",
      vars: { balance: "50,00", currency: "EUR" },
    });
  });

  // CSH-031 — generic backend error code surfaced as error.<code>
  it("surfaces generic backend error code", async () => {
    mockRecordWithdrawal.mockResolvedValue({
      status: "error",
      error: { code: "AmountNotPositive" },
    });
    const { result } = renderHook(() => useWithdrawalTransaction({ accountId: "account-1" }));

    act(() => result.current.handleChange("amount", "100"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.AmountNotPositive" });
  });

  // DatabaseError path — the presenter maps to error.DatabaseError; logger keeps
  // the full payload server-side via tracing for triage outside the user-visible message.
  it("logs full error and maps DatabaseError to inline i18n key", async () => {
    mockRecordWithdrawal.mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const { result } = renderHook(() => useWithdrawalTransaction({ accountId: "account-1" }));

    act(() => result.current.handleChange("amount", "100"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(logger.error).toHaveBeenCalledWith("[useWithdrawalTransaction] submit failed", {
      error: { code: "DatabaseError" },
    });
    expect(mockShowSnackbar).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// CSH-111 — edit mode: prefill from the existing Withdrawal + persist via
// correct_transaction (not record_withdrawal), with the "updated" snackbar.
// ---------------------------------------------------------------------------
const editWithdrawal = {
  id: "tx-wd-1",
  account_id: "account-1",
  asset_id: "system-cash-eur",
  transaction_type: "Withdrawal",
  date: "2026-05-10",
  quantity: 120_000_000,
  unit_price: 1_000_000,
  exchange_rate: 1_000_000,
  fees: 0,
  total_amount: 120_000_000,
  note: null,
  realized_pnl: null,
  created_at: "2026-05-10T00:00:00Z",
} as const;

describe("useWithdrawalTransaction — edit mode (CSH-111)", () => {
  beforeEach(() => {
    mockRecordWithdrawal.mockReset();
    mockCorrectTransaction.mockReset();
    mockShowSnackbar.mockReset();
  });

  it("prefills the form from the edited Withdrawal (date, amount)", () => {
    const { result } = renderHook(() =>
      useWithdrawalTransaction({ accountId: "account-1", editTransaction: editWithdrawal }),
    );
    expect(result.current.formData.date).toBe("2026-05-10");
    expect(result.current.formData.amount).toBe("120.000");
    expect(result.current.formData.note).toBe("");
  });

  it("submits via correctTransaction (not recordWithdrawal) and shows the updated snackbar", async () => {
    mockCorrectTransaction.mockResolvedValue({ status: "ok", data: { id: "tx-wd-1" } });
    const { result } = renderHook(() =>
      useWithdrawalTransaction({ accountId: "account-1", editTransaction: editWithdrawal }),
    );

    act(() => result.current.handleChange("amount", "90"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordWithdrawal).not.toHaveBeenCalled();
    expect(mockCorrectTransaction).toHaveBeenCalledWith(
      "tx-wd-1",
      "account-1",
      expect.objectContaining({ date: "2026-05-10", quantity: 90_000_000 }),
    );
    expect(mockShowSnackbar).toHaveBeenCalledWith("cash.withdrawal_updated", "success");
  });

  it("falls back to the Unknown error when the gateway throws", async () => {
    mockCorrectTransaction.mockRejectedValue(new Error("ipc down"));
    const { result } = renderHook(() =>
      useWithdrawalTransaction({ accountId: "account-1", editTransaction: editWithdrawal }),
    );

    act(() => result.current.handleChange("amount", "10"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.Unknown" });
  });
});
