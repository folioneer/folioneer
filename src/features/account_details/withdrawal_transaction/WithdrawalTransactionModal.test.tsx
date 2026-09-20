import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Transaction } from "@/bindings";
import { WithdrawalTransactionModal } from "./WithdrawalTransactionModal";

const { mockUseWithdrawalTransaction } = vi.hoisted(() => ({
  mockUseWithdrawalTransaction: vi.fn(),
}));

vi.mock("./useWithdrawalTransaction", () => ({
  useWithdrawalTransaction: (...args: unknown[]) => mockUseWithdrawalTransaction(...args),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

const makeHookReturn = (overrides: Record<string, unknown> = {}) => ({
  formData: { date: "2026-05-10", amount: "", note: "" },
  error: null,
  isSubmitting: false,
  isFormValid: false,
  handleChange: vi.fn(),
  handleSubmit: vi.fn(),
  ...overrides,
});

const BASE_PROPS = {
  isOpen: true,
  onClose: vi.fn(),
  accountId: "acc-1",
  accountName: "Main",
  accountCurrency: "EUR",
  onSubmitSuccess: vi.fn(),
};

const editTx = { id: "tx-wd-1", total_amount: 120_000_000 } as unknown as Transaction;

describe("WithdrawalTransactionModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseWithdrawalTransaction.mockReturnValue(makeHookReturn());
  });

  it("renders the record-mode title and action label by default (CSH-030)", () => {
    render(<WithdrawalTransactionModal {...BASE_PROPS} />);
    expect(screen.getByText("cash.withdrawal_modal_title")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /cash\.action_record_withdrawal/i }),
    ).toBeInTheDocument();
    expect(document.querySelector("#withdrawal-trx-amount")).toBeInTheDocument();
  });

  it("renders the edit-mode title and Save label when editing (CSH-111)", () => {
    render(<WithdrawalTransactionModal {...BASE_PROPS} editTransaction={editTx} />);
    expect(screen.getByText("cash.withdrawal_edit_modal_title")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /action\.save/i })).toBeInTheDocument();
    expect(mockUseWithdrawalTransaction).toHaveBeenCalledWith(
      expect.objectContaining({ editTransaction: editTx }),
    );
  });

  it("renders an alert when the hook reports an error", () => {
    mockUseWithdrawalTransaction.mockReturnValue(
      makeHookReturn({ error: { key: "cash.insufficient_cash_inline" } }),
    );
    render(<WithdrawalTransactionModal {...BASE_PROPS} />);
    expect(screen.getByRole("alert")).toHaveTextContent("cash.insufficient_cash_inline");
  });
});
