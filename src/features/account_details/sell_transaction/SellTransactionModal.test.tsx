import { fireEvent, render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SellTransactionModal } from "./SellTransactionModal";

const { mockUseSellTransaction } = vi.hoisted(() => ({
  mockUseSellTransaction: vi.fn(),
}));

vi.mock("./useSellTransaction", () => ({
  useSellTransaction: (...args: unknown[]) => mockUseSellTransaction(...args),
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
  maxQuantityDisplay: "3.000000",
  entryMode: "price",
  setEntryMode: vi.fn(),
  totalAmountInput: "",
  handleTotalAmountChange: vi.fn(),
  fieldErrors: {},
  problemHint: null,
  unitPriceDisplay: "—",
  averageCostAsOfDate: null,
  potentialPnl: null,
  error: null,
  isSubmitting: false,
  isFormValid: false,
  recordPrice: false,
  setRecordPrice: vi.fn(),
  handleChange: vi.fn(),
  handleSubmit: vi.fn(),
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
  assetReference: "AAPL",
  holdingQuantity: "3",
  onSubmitSuccess: vi.fn(),
};

describe("SellTransactionModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseSellTransaction.mockReturnValue(makeHookReturn());
  });

  // SEL-050 — price mode (default): unit price editable, total read-only
  it("renders an editable unit price and a read-only total in price mode", () => {
    render(<SellTransactionModal {...BASE_PROPS} />);

    const unitPrice = document.querySelector("#sell-trx-unit-price") as HTMLInputElement;
    const total = document.querySelector("#sell-trx-total") as HTMLInputElement;
    expect(unitPrice).not.toHaveAttribute("readonly");
    expect(total).toHaveAttribute("readonly");

    expect(document.querySelector("#sell-trx-entry-mode-price")).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  // SEL-050 — total mode: total editable, unit price becomes a read-only derived display
  it("swaps to an editable total and a read-only derived unit price in total mode", () => {
    mockUseSellTransaction.mockReturnValue(
      makeHookReturn({ entryMode: "total", unitPriceDisplay: "150.000" }),
    );
    render(<SellTransactionModal {...BASE_PROPS} />);

    const unitPrice = document.querySelector("#sell-trx-unit-price") as HTMLInputElement;
    const total = document.querySelector("#sell-trx-total") as HTMLInputElement;
    expect(unitPrice).toHaveAttribute("readonly");
    expect(unitPrice.value).toBe("150.000");
    expect(total).not.toHaveAttribute("readonly");

    expect(document.querySelector("#sell-trx-entry-mode-total")).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  // SEL-050 — clicking a toggle segment switches the mode
  it("calls setEntryMode when a toggle segment is clicked", () => {
    const setEntryMode = vi.fn();
    mockUseSellTransaction.mockReturnValue(makeHookReturn({ setEntryMode }));
    render(<SellTransactionModal {...BASE_PROPS} />);

    fireEvent.click(document.querySelector("#sell-trx-entry-mode-total") as HTMLElement);
    expect(setEntryMode).toHaveBeenCalledWith("total");
  });
});

// TDI-032 (#812) — a potential P&L of zero is neither a gain nor a loss: neutral colour.
describe("SellTransactionModal — the potential P&L colour (TDI-032)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it.each([
    [1_000_000, "text-m3-gain"],
    [-1_000_000, "text-m3-loss"],
    [0, "text-m3-on-surface-variant"],
  ])("colours a potential P&L of %i with %s", (raw, colour) => {
    mockUseSellTransaction.mockReturnValue(
      makeHookReturn({ potentialPnl: { raw, formatted: "x" } }),
    );
    render(<SellTransactionModal {...BASE_PROPS} />);
    expect(document.querySelector("#sell-trx-potential-pnl")?.className).toContain(colour);
  });
});
