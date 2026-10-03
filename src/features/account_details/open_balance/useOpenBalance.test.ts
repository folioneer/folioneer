import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { OpeningBalanceDraft } from "@/bindings";
import { logger } from "@/lib/logger";
import { useOpenBalance } from "./useOpenBalance";

// ── Gateway mock ──────────────────────────────────────────────────────────────
// vi.hoisted ensures the spy references exist before vi.mock is hoisted.
const { mockOpenHolding, mockValidateDraft } = vi.hoisted(() => ({
  mockOpenHolding: vi.fn(),
  mockValidateDraft: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    openHolding: (...args: unknown[]) => mockOpenHolding(...args),
    validateOpeningBalanceDraft: (...args: unknown[]) => mockValidateDraft(...args),
  },
}));

// The core's opening balance draft check (TRX-066), standing in for the backend: the
// first problem, or whether a zero cost calls for the warning (TRX-065).
const coreCheck = async (draft: OpeningBalanceDraft) => {
  const problem = (code: string) => ({ status: "error" as const, error: { code } });
  if (!draft.date) return problem("DateMissing");
  if (draft.total_cost === null) return problem("TotalCostMissing");
  if (draft.quantity <= 0) return problem("QuantityNotPositive");
  if (draft.total_cost < 0) return problem("InvalidTotalCost");
  return { status: "ok" as const, data: { zero_cost: draft.total_cost === 0 } };
};

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() },
}));

const { mockShowSnackbar } = vi.hoisted(() => ({
  mockShowSnackbar: vi.fn(),
}));

vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en" },
  }),
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────
const BASE_PROPS = {
  accountId: "account-1",
  assetId: "asset-1",
};

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

const makeTransaction = () => ({
  id: "tx-open-1",
  account_id: "account-1",
  asset_id: "asset-1",
  transaction_type: "OpeningBalance" as const,
  date: "2024-01-15",
  quantity: 5_000_000,
  unit_price: 100_000_000,
  exchange_rate: 1_000_000,
  fees: 0,
  total_amount: 500_000_000,
  note: null,
  realized_pnl: null,
  created_at: "2024-01-15T10:00:00Z",
});

describe("useOpenBalance", () => {
  beforeEach(() => {
    mockOpenHolding.mockReset();
    mockShowSnackbar.mockReset();
    mockValidateDraft.mockReset();
    mockValidateDraft.mockImplementation(coreCheck);
  });

  // ── Initial state ─────────────────────────────────────────────────────────

  // TRX-011 — account_id and asset_id are pre-filled from props (read-only context)
  it("initialises formData accountId and assetId from props", () => {
    const { result } = renderHook(() => useOpenBalance({ accountId: "acc-42", assetId: "ast-99" }));
    expect(result.current.formData.accountId).toBe("acc-42");
    expect(result.current.formData.assetId).toBe("ast-99");
  });

  // Default form values: date=today, quantity and totalCost empty
  it("initialises date to today and quantity/totalCost to empty string", () => {
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));
    const expectedToday = new Date().toISOString().slice(0, 10);
    expect(result.current.formData.date).toBe(expectedToday);
    expect(result.current.formData.quantity).toBe("");
    expect(result.current.formData.totalCost).toBe("");
  });

  // isFormValid is false on initial render (quantity and totalCost are empty)
  it("isFormValid is false on initial render", () => {
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));
    expect(result.current.isFormValid).toBe(false);
  });

  // TRX-066 — the draft goes to the core in micro-units; a total cost not typed yet is
  // sent as none, never as 0, so an untouched form raises no zero-cost warning.
  it("sends the draft to the core, an empty total cost as none", async () => {
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));
    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "5");
    });

    expect(mockValidateDraft).toHaveBeenLastCalledWith({
      account_id: "account-1",
      asset_id: "asset-1",
      date: "2024-01-15",
      quantity: 5_000_000,
      total_cost: null,
    });
    expect(result.current.isFormValid).toBe(false);
    expect(result.current.zeroCostWarning).toBe(false);
  });

  // TRX-066 — saving follows the core's answer, whatever the form holds.
  it("keeps saving disabled while the core reports a problem or cannot be asked", async () => {
    mockValidateDraft.mockResolvedValue({ status: "error", error: { code: "DateInFuture" } });
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));
    await act(async () => {
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "500");
    });
    expect(result.current.isFormValid).toBe(false);

    mockValidateDraft.mockRejectedValue(new Error("ipc down"));
    await act(async () => {
      result.current.handleChange("totalCost", "600");
    });
    expect(result.current.isFormValid).toBe(false);
    expect(logger.error).toHaveBeenCalled();
    expect(result.current.error).toEqual({ key: "error.Unknown" });

    mockValidateDraft.mockImplementation(coreCheck);
    await act(async () => {
      result.current.handleChange("totalCost", "700");
    });
    expect(result.current.error).toBeNull();
    expect(result.current.isFormValid).toBe(true);
  });

  // ── handleChange ─────────────────────────────────────────────────────────

  it("handleChange updates quantity field in formData", async () => {
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("quantity", "5");
    });

    expect(result.current.formData.quantity).toBe("5");
  });

  it("handleChange updates totalCost field in formData", async () => {
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("totalCost", "500");
    });

    expect(result.current.formData.totalCost).toBe("500");
  });

  // ── Validation ────────────────────────────────────────────────────────────

  // TRX-043 — OpeningBalance form has no fees, no exchange_rate, no unit_price
  // (verified by hook exposing only date, quantity, totalCost in formData)
  it("formData does not contain fees, exchangeRate, or unitPrice fields (TRX-043)", () => {
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));
    expect(result.current.formData).not.toHaveProperty("fees");
    expect(result.current.formData).not.toHaveProperty("exchangeRate");
    expect(result.current.formData).not.toHaveProperty("unitPrice");
  });

  // isFormValid true when date, quantity > 0, and totalCost > 0 are set
  it("isFormValid is true when date, quantity and totalCost are all positive", async () => {
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "500");
    });

    expect(result.current.isFormValid).toBe(true);
  });

  // TRX-044 — quantity = 0 is invalid
  it("isFormValid is false when quantity is zero (TRX-044)", async () => {
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "0");
      result.current.handleChange("totalCost", "500");
    });

    expect(result.current.isFormValid).toBe(false);
  });

  // TRX-045 — totalCost = 0 is valid (zero-cost position: mined / gifted / airdropped)
  it("isFormValid is true when totalCost is zero (TRX-045)", async () => {
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "0");
    });

    expect(result.current.isFormValid).toBe(true);
  });

  // TRX-045 — a negative totalCost is still invalid
  it("isFormValid is false when totalCost is negative (TRX-045)", async () => {
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "-10");
    });

    expect(result.current.isFormValid).toBe(false);
  });

  // Date required — empty date is invalid
  it("isFormValid is false when date is empty", async () => {
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("date", "");
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "500");
    });

    expect(result.current.isFormValid).toBe(false);
  });

  // ── Successful submit ─────────────────────────────────────────────────────

  // TRX-042 — submit calls openHolding with micro-unit DTO and invokes onSubmitSuccess
  it("handleSubmit calls openHolding with correct micro-unit DTO on success", async () => {
    mockOpenHolding.mockResolvedValue({
      status: "ok",
      data: makeTransaction(),
    });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useOpenBalance({ ...BASE_PROPS, onSubmitSuccess }));

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "500");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockOpenHolding).toHaveBeenCalledWith(
      expect.objectContaining({
        account_id: "account-1",
        asset_id: "asset-1",
        date: "2024-01-15",
        quantity: 5_000_000,
        total_cost: 500_000_000,
      }),
    );
    expect(onSubmitSuccess).toHaveBeenCalledTimes(1);
  });

  // Success clears the error state
  it("clears error on successful submit", async () => {
    mockOpenHolding.mockResolvedValue({
      status: "ok",
      data: makeTransaction(),
    });
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "500");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toBeNull();
  });

  // ── Error paths ───────────────────────────────────────────────────────────

  // TRX-044 — QuantityNotPositive: backend error sets inline error, does not call onSubmitSuccess
  it("sets error key on QuantityNotPositive and does not call onSubmitSuccess", async () => {
    mockOpenHolding.mockResolvedValue({
      status: "error",
      error: { code: "QuantityNotPositive" },
    });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useOpenBalance({ ...BASE_PROPS, onSubmitSuccess }));

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "500");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.QuantityNotPositive" });
    expect(onSubmitSuccess).not.toHaveBeenCalled();
  });

  // TRX-045 — InvalidTotalCost: backend error sets inline error, does not call onSubmitSuccess
  it("sets error key on InvalidTotalCost and does not call onSubmitSuccess", async () => {
    mockOpenHolding.mockResolvedValue({
      status: "error",
      error: { code: "InvalidTotalCost" },
    });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useOpenBalance({ ...BASE_PROPS, onSubmitSuccess }));

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "500");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.InvalidTotalCost" });
    expect(onSubmitSuccess).not.toHaveBeenCalled();
  });

  // TRX-050 — ArchivedAsset: backend error sets inline error, does not call onSubmitSuccess
  it("sets error key on ArchivedAsset and does not call onSubmitSuccess", async () => {
    mockOpenHolding.mockResolvedValue({
      status: "error",
      error: { code: "ArchivedAsset" },
    });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useOpenBalance({ ...BASE_PROPS, onSubmitSuccess }));

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "500");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.ArchivedAsset" });
    expect(onSubmitSuccess).not.toHaveBeenCalled();
  });

  // Generic backend error: sets error from error code, modal stays open.
  // Also asserts the diagnostic `hint` flows through to logger.error so support
  // reports retain the developer-only triage info that `error.Unknown` hides.
  it("sets DatabaseError and does not call onSubmitSuccess on generic backend error", async () => {
    mockOpenHolding.mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useOpenBalance({ ...BASE_PROPS, onSubmitSuccess }));

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "500");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(onSubmitSuccess).not.toHaveBeenCalled();
    expect(logger.error).toHaveBeenCalledWith("[useOpenBalance] openHolding failed", {
      error: { code: "DatabaseError" },
    });
  });

  // TRX-058 — on success: snackbar fires with success_created key + onSubmitSuccess is called
  it("TRX-058: calls showSnackbar with success key and calls onSubmitSuccess on success", async () => {
    mockOpenHolding.mockResolvedValue({
      status: "ok",
      data: makeTransaction(),
    });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useOpenBalance({ ...BASE_PROPS, onSubmitSuccess }));

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "500");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("open_balance.success_created", "success");
    expect(onSubmitSuccess).toHaveBeenCalledTimes(1);
  });

  // isSubmitting is true while gateway call is in-flight
  it("isSubmitting is true during in-flight gateway call", async () => {
    let resolveCall!: () => void;
    mockOpenHolding.mockReturnValue(
      new Promise<{ status: string; data: ReturnType<typeof makeTransaction> }>((resolve) => {
        resolveCall = () => resolve({ status: "ok", data: makeTransaction() });
      }),
    );
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "5");
      result.current.handleChange("totalCost", "500");
    });

    let submitPromise: Promise<void>;
    act(() => {
      submitPromise = result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.isSubmitting).toBe(true);

    await act(async () => {
      resolveCall();
      await submitPromise;
    });

    expect(result.current.isSubmitting).toBe(false);
  });

  // TRX-065 — the core says when to warn: a total cost of exactly 0 on a draft that can be
  // recorded; empty or positive does not, nor does a draft with a problem.
  it("warns when the core flags a zero cost", async () => {
    const { result } = renderHook(() => useOpenBalance(BASE_PROPS));
    expect(result.current.zeroCostWarning).toBe(false);

    await act(async () => result.current.handleChange("totalCost", "0"));
    expect(result.current.zeroCostWarning).toBe(false);

    await act(async () => result.current.handleChange("quantity", "5"));
    expect(result.current.zeroCostWarning).toBe(true);

    await act(async () => result.current.handleChange("totalCost", "0.00"));
    expect(result.current.zeroCostWarning).toBe(true);

    await act(async () => result.current.handleChange("totalCost", "12.5"));
    expect(result.current.zeroCostWarning).toBe(false);
  });
});
