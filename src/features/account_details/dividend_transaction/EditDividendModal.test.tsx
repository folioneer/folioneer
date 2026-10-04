import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { EditDividendModal } from "./EditDividendModal";

const { mockUseEditDividend } = vi.hoisted(() => ({ mockUseEditDividend: vi.fn() }));

vi.mock("./useEditDividend", () => ({ useEditDividend: () => mockUseEditDividend() }));
vi.mock("@/lib/logger", () => ({ logger: { error: vi.fn(), info: vi.fn() } }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
    i18n: { language: "en" },
  }),
}));

const makeState = (overrides: Record<string, unknown> = {}) => ({
  formData: { date: "2026-09-15", amount: "18.6", exchangeRate: "0.9214", note: "Interim" },
  totalDisplay: "17.14",
  fieldErrors: {},
  problemHint: null,
  error: null,
  isSubmitting: false,
  isFormValid: true,
  handleChange: vi.fn(),
  handleSubmit: vi.fn((event: React.FormEvent) => event.preventDefault()),
  ...overrides,
});

const dividend = {
  transactionId: "tx-div-1",
  accountId: "account-1",
  assetId: "asset-1",
  date: "2026-09-15",
  amount: "18.6",
  exchangeRate: "0.9214",
  note: "Interim",
  unitPriceMicro: 1_000_000,
};

const renderModal = (assetCurrency: string, amountInAccountCurrency = false) =>
  render(
    <EditDividendModal
      onClose={vi.fn()}
      onSubmitSuccess={vi.fn()}
      dividend={dividend}
      accountName="PEA"
      accountCurrency="EUR"
      assetName="Microsoft"
      assetCurrency={assetCurrency}
      amountInAccountCurrency={amountInAccountCurrency}
    />,
  );

describe("EditDividendModal (DIV-042)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseEditDividend.mockReturnValue(makeState());
  });

  // A dividend has a date, an amount, a note and a total — and none of what a trade has.
  it("shows what a dividend has, and no unit price, fees or market-price checkbox", () => {
    renderModal("EUR");

    for (const id of ["date", "amount", "total", "note", "save", "cancel"]) {
      expect(document.getElementById(`edit-dividend-${id}`)).not.toBeNull();
    }
    expect(document.querySelector('[id*="unit-price"]')).toBeNull();
    expect(document.querySelector('[id*="fees"]')).toBeNull();
    expect(document.querySelector('input[type="checkbox"]')).toBeNull();
    expect(screen.getByLabelText("dividend.form_amount_label (EUR)")).toHaveValue("18.6");
  });

  // DIV-022 — the exchange rate is asked only when the paying asset and the account
  // differ in currency.
  it("asks for the exchange rate only when the currencies differ", () => {
    const same = renderModal("EUR");
    expect(document.getElementById("edit-dividend-exchange-rate")).toBeNull();
    same.unmount();

    renderModal("USD");
    expect(document.getElementById("edit-dividend-exchange-rate")).toHaveValue("0.9214");
    expect(screen.getByLabelText("dividend.form_amount_label (USD)")).toBeInTheDocument();
  });

  // DIV-043 — a dividend recorded in the account's currency is corrected in that mode: its
  // amount is labelled in the account's currency and no rate is asked, although the
  // paying asset is in another currency.
  it("keeps the account-currency mode of a dividend recorded that way", () => {
    renderModal("USD", true);

    expect(screen.getByLabelText("dividend.form_amount_label (EUR)")).toBeInTheDocument();
    expect(document.getElementById("edit-dividend-exchange-rate")).toBeNull();
  });

  // DIV-040 — the account and the paying asset are stated, not editable.
  it("states the paying asset and the account without offering to change them", () => {
    renderModal("USD");

    expect(document.getElementById("edit-dividend-asset")).toHaveTextContent("Microsoft");
    expect(document.getElementById("edit-dividend-locked-note")).toHaveTextContent(
      'dividend.edit_locked_note:{"account":"PEA","currency":"EUR"}',
    );
    expect(document.querySelector("select")).toBeNull();
    expect(document.querySelector('[role="combobox"]')).toBeNull();
  });

  // TRX-062 — the total is the core's, in the account's currency, and cannot be typed.
  it("shows the core's total, read-only, and a dash while it is unknown", () => {
    const known = renderModal("USD");
    const total = document.getElementById("edit-dividend-total");
    expect(total).toHaveValue("17.14 EUR");
    expect(total).toHaveAttribute("readonly");
    known.unmount();

    mockUseEditDividend.mockReturnValue(makeState({ totalDisplay: null, isFormValid: false }));
    renderModal("USD");
    expect(document.getElementById("edit-dividend-total")).toHaveValue("—");
    expect(document.getElementById("edit-dividend-save")).toBeDisabled();
  });

  // TRX-067 — a problem shows under its field; one that concerns no field, beside the
  // actions.
  it("shows a problem on its field and a refusal beside the actions", () => {
    mockUseEditDividend.mockReturnValue(
      makeState({
        fieldErrors: { quantity: { key: "transaction.error_validation_quantity" } },
        error: { key: "error.InsufficientCash" },
        isFormValid: false,
      }),
    );
    renderModal("USD");

    expect(document.getElementById("edit-dividend-amount-error")).toHaveTextContent(
      "transaction.error_validation_quantity",
    );
    expect(document.getElementById("edit-dividend-error")).toHaveTextContent(
      "error.InsufficientCash",
    );
  });

  it("does not offer to save while the check is not clean or a save is running", () => {
    mockUseEditDividend.mockReturnValue(makeState({ isFormValid: false }));
    const invalid = renderModal("USD");
    expect(document.getElementById("edit-dividend-save")).toBeDisabled();
    invalid.unmount();

    mockUseEditDividend.mockReturnValue(makeState({ isSubmitting: true }));
    renderModal("USD");
    expect(document.getElementById("edit-dividend-save")).toBeDisabled();
    expect(document.getElementById("edit-dividend-cancel")).toBeDisabled();
  });

  it("passes what is typed to the hook and submits through the form", () => {
    const state = makeState();
    mockUseEditDividend.mockReturnValue(state);
    renderModal("USD");

    fireEvent.change(document.getElementById("edit-dividend-note") as HTMLElement, {
      target: { value: "Final" },
    });
    expect(state.handleChange).toHaveBeenCalledWith("note", "Final");

    fireEvent.submit(document.getElementById("edit-dividend-form") as HTMLElement);
    expect(state.handleSubmit).toHaveBeenCalledTimes(1);
  });
});
