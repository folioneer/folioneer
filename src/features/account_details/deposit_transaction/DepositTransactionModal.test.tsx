import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Transaction } from "@/bindings";
import { DepositTransactionModal } from "./DepositTransactionModal";

const { mockUseDepositTransaction } = vi.hoisted(() => ({
  mockUseDepositTransaction: vi.fn(),
}));

vi.mock("./useDepositTransaction", () => ({
  useDepositTransaction: (...args: unknown[]) => mockUseDepositTransaction(...args),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

const makeHookReturn = (overrides: Record<string, unknown> = {}) => ({
  formData: { date: "2026-05-01", amount: "", note: "" },
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

const editTx = { id: "tx-dep-1", total_amount: 500_000_000 } as unknown as Transaction;

describe("DepositTransactionModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseDepositTransaction.mockReturnValue(makeHookReturn());
  });

  it("renders the record-mode title and action label by default (CSH-020)", () => {
    render(<DepositTransactionModal {...BASE_PROPS} />);
    expect(screen.getByText("cash.deposit_modal_title")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /cash\.action_record_deposit/i }),
    ).toBeInTheDocument();
    expect(document.querySelector("#deposit-trx-amount")).toBeInTheDocument();
  });

  it("renders the edit-mode title and Save label when editing (CSH-111)", () => {
    render(<DepositTransactionModal {...BASE_PROPS} editTransaction={editTx} />);
    expect(screen.getByText("cash.deposit_edit_modal_title")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /action\.save/i })).toBeInTheDocument();
    // The hook receives the edit transaction.
    expect(mockUseDepositTransaction).toHaveBeenCalledWith(
      expect.objectContaining({ editTransaction: editTx }),
    );
  });

  it("renders an alert when the hook reports an error", () => {
    mockUseDepositTransaction.mockReturnValue(
      makeHookReturn({ error: { key: "error.DatabaseError" } }),
    );
    render(<DepositTransactionModal {...BASE_PROPS} />);
    expect(screen.getByRole("alert")).toHaveTextContent("error.DatabaseError");
  });
});
