import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { logger } from "@/lib/logger";
import { useManagementFeeEdit } from "./useManagementFeeEdit";

// ── Hoisted mocks ──────────────────────────────────────────────────────────────
const { mockCorrectTransaction, mockShowSnackbar } = vi.hoisted(() => ({
  mockCorrectTransaction: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
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
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

// ── Fixtures ───────────────────────────────────────────────────────────────────
const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

const EDIT_CONTEXT = {
  transactionId: "tx-fee-1",
  lockedAssetName: "Apple Inc",
  initialDate: "2024-06-15",
  initialQuantity: "1.000",
  initialNote: "Q2 fee",
};

const BASE_PROPS = {
  accountId: "account-1",
  editContext: EDIT_CONTEXT,
  onSubmitSuccess: vi.fn(),
};

// ── Tests ──────────────────────────────────────────────────────────────────────
describe("useManagementFeeEdit (FEE-063)", () => {
  beforeEach(() => {
    mockCorrectTransaction.mockReset();
    mockShowSnackbar.mockReset();
    vi.mocked(logger.error).mockClear();
    BASE_PROPS.onSubmitSuccess.mockClear();
  });

  it("prefills the form from the edit context", () => {
    const { result } = renderHook(() => useManagementFeeEdit(BASE_PROPS));
    expect(result.current.formData.date).toBe("2024-06-15");
    expect(result.current.formData.quantity).toBe("1.000");
    expect(result.current.formData.note).toBe("Q2 fee");
    expect(result.current.isFormValid).toBe(true);
  });

  it("is invalid when the removed quantity is not strictly positive", () => {
    const { result } = renderHook(() => useManagementFeeEdit(BASE_PROPS));
    act(() => result.current.handleChange("quantity", "0"));
    expect(result.current.isFormValid).toBe(false);
  });

  it("submits via correct_transaction with the zero-cost money convention", async () => {
    mockCorrectTransaction.mockResolvedValue({ status: "ok", data: {} });
    const { result } = renderHook(() => useManagementFeeEdit(BASE_PROPS));

    act(() => result.current.handleChange("quantity", "2.5"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockCorrectTransaction).toHaveBeenCalledWith("tx-fee-1", "account-1", {
      date: "2024-06-15",
      quantity: 2_500_000,
      unit_price: 0,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: null,
      note: "Q2 fee",
    });
    expect(mockShowSnackbar).toHaveBeenCalledWith("management_fee.updated", "success");
    expect(BASE_PROPS.onSubmitSuccess).toHaveBeenCalledTimes(1);
  });

  it("maps a backend rejection to an i18n error and does not call onSubmitSuccess", async () => {
    mockCorrectTransaction.mockResolvedValue({
      status: "error",
      error: { code: "CascadingOversell" },
    });
    const { result } = renderHook(() => useManagementFeeEdit(BASE_PROPS));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.CascadingOversell" });
    expect(mockShowSnackbar).not.toHaveBeenCalled();
    expect(BASE_PROPS.onSubmitSuccess).not.toHaveBeenCalled();
  });
});
