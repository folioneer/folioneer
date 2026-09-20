import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BuyTransactionModal } from "./BuyTransactionModal";

const { mockUseBuyTransaction } = vi.hoisted(() => ({
  mockUseBuyTransaction: vi.fn(),
}));

vi.mock("./useBuyTransaction", () => ({
  useBuyTransaction: (...args: unknown[]) => mockUseBuyTransaction(...args),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

const makeHookReturn = (overrides: Record<string, unknown> = {}) => ({
  formData: {
    accountId: "acc-1",
    assetId: "asset-1",
    date: "2026-06-01",
    quantity: "",
    unitPrice: "",
    exchangeRate: "1.000000",
    fees: "0",
    note: "",
  },
  totalAmountDisplay: "0.000",
  entryMode: "price",
  setEntryMode: vi.fn(),
  totalAmountInput: "",
  handleTotalAmountChange: vi.fn(),
  totalBelowFeesError: null,
  unitPriceDisplay: "—",
  averageCostAsOfDate: null,
  error: null,
  isSubmitting: false,
  isFormValid: false,
  showArchivedConfirm: false,
  recordPrice: false,
  setRecordPrice: vi.fn(),
  handleChange: vi.fn(),
  handleSubmit: vi.fn(),
  handleConfirmArchived: vi.fn(),
  handleCancelArchived: vi.fn(),
  ...overrides,
});

const BASE_PROPS = {
  isOpen: true,
  onClose: vi.fn(),
  accountId: "acc-1",
  accountName: "Main",
  assetId: "asset-1",
  assetName: "Apple",
  assetCurrency: "USD",
  onSubmitSuccess: vi.fn(),
};

describe("BuyTransactionModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseBuyTransaction.mockReturnValue(makeHookReturn());
  });

  // TRX-060 — price mode (default): unit price editable, total read-only
  it("renders an editable unit price and a read-only total in price mode", () => {
    render(<BuyTransactionModal {...BASE_PROPS} />);

    const unitPrice = document.querySelector("#buy-trx-unit-price") as HTMLInputElement;
    const total = document.querySelector("#buy-trx-total") as HTMLInputElement;
    expect(unitPrice).not.toHaveAttribute("readonly");
    expect(total).toHaveAttribute("readonly");

    expect(document.querySelector("#buy-trx-entry-mode-price")).toHaveAttribute(
      "aria-checked",
      "true",
    );
    expect(document.querySelector("#buy-trx-entry-mode-total")).toHaveAttribute(
      "aria-checked",
      "false",
    );
  });

  // TRX-060 — total mode: total editable, unit price becomes a read-only derived display
  it("swaps to an editable total and a read-only derived unit price in total mode", () => {
    mockUseBuyTransaction.mockReturnValue(
      makeHookReturn({ entryMode: "total", unitPriceDisplay: "100.000" }),
    );
    render(<BuyTransactionModal {...BASE_PROPS} />);

    const unitPrice = document.querySelector("#buy-trx-unit-price") as HTMLInputElement;
    const total = document.querySelector("#buy-trx-total") as HTMLInputElement;
    expect(unitPrice).toHaveAttribute("readonly");
    expect(unitPrice.value).toBe("100.000");
    expect(total).not.toHaveAttribute("readonly");

    expect(document.querySelector("#buy-trx-entry-mode-total")).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  // TRX-060 — the below-fees rejection renders inline on the Total field in total mode
  it("renders the below-fees error on the total field in total mode", () => {
    mockUseBuyTransaction.mockReturnValue(
      makeHookReturn({
        entryMode: "total",
        totalAmountInput: "5",
        totalBelowFeesError: { key: "transaction.error_validation_total_below_fees" },
      }),
    );
    render(<BuyTransactionModal {...BASE_PROPS} />);

    expect(screen.getByText("transaction.error_validation_total_below_fees")).toBeInTheDocument();
  });

  // TRX-060 — clicking a toggle segment switches the mode
  it("calls setEntryMode when a toggle segment is clicked", () => {
    const setEntryMode = vi.fn();
    mockUseBuyTransaction.mockReturnValue(makeHookReturn({ setEntryMode }));
    render(<BuyTransactionModal {...BASE_PROPS} />);

    fireEvent.click(document.querySelector("#buy-trx-entry-mode-total") as HTMLElement);
    expect(setEntryMode).toHaveBeenCalledWith("total");
  });
});
