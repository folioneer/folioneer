import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DividendTransactionModal } from "./DividendTransactionModal";

// ── Mock the hook that drives the component ────────────────────────────────────
const { mockUseDividendTransaction } = vi.hoisted(() => ({
  mockUseDividendTransaction: vi.fn(),
}));

vi.mock("./useDividendTransaction", () => ({
  useDividendTransaction: (...args: unknown[]) => mockUseDividendTransaction(...args),
}));

// ComboboxField cannot be driven via jsdom events (HeadlessUI), so stub it: expose
// the item ids it received and let a click select the first item (ADR-007 boundary).
vi.mock("@/ui/components/field/ComboboxField", () => ({
  ComboboxField: ({
    id,
    items,
    idKey,
    onChange,
  }: {
    id: string;
    items: Record<string, unknown>[];
    idKey: string;
    onChange: (value: string) => void;
  }) => (
    <button
      type="button"
      data-testid={`combobox-${id}`}
      data-item-ids={items.map((item) => String(item[idKey])).join(",")}
      onClick={() => onChange(String(items[0]?.[idKey]))}
    />
  ),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en" },
  }),
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() },
}));

// ── Shared held assets ─────────────────────────────────────────────────────────
const heldAssets = [
  { assetId: "asset-eur-1", assetName: "Apple Inc", assetCurrency: "EUR" },
  { assetId: "asset-usd-1", assetName: "Tesla Inc", assetCurrency: "USD" },
];

// ── Shared hook return factory ─────────────────────────────────────────────────
const TODAY = new Date().toISOString().slice(0, 10);

const makeHookReturn = (overrides: Record<string, unknown> = {}) => ({
  formData: {
    assetId: "",
    date: TODAY,
    amount: "",
    exchangeRate: "1.000000",
    note: "",
  },
  error: null,
  isSubmitting: false,
  isFormValid: false,
  showExchangeRate: false,
  showCurrencyModeSwitch: false,
  amountInAccountCurrency: false,
  setAmountInAccountCurrency: vi.fn(),
  amountCurrency: "EUR",
  handleChange: vi.fn(),
  handleSubmit: vi.fn(),
  ...overrides,
});

// ── Shared component props ─────────────────────────────────────────────────────
const BASE_PROPS = {
  isOpen: true,
  onClose: vi.fn(),
  accountId: "account-1",
  accountCurrency: "EUR",
  heldAssets,
  onSubmitSuccess: vi.fn(),
  onRecorded: vi.fn(),
};

describe("DividendTransactionModal (DIV-020/021/022/025)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseDividendTransaction.mockReturnValue(makeHookReturn());
  });

  // DIV-020 — asset selector receives all held assets (F25 stable id)
  it("renders the asset selector with all held assets (DIV-020)", () => {
    render(<DividendTransactionModal {...BASE_PROPS} />);
    const combobox = screen.getByTestId("combobox-dividend-trx-asset");
    expect(combobox).toBeInTheDocument();
    expect(combobox.getAttribute("data-item-ids")).toBe("asset-eur-1,asset-usd-1");
  });

  // DIV-020 — date field present (F25 stable id)
  it("renders a date field with stable id (DIV-020)", () => {
    render(<DividendTransactionModal {...BASE_PROPS} />);
    expect(screen.getByTestId("dividend-trx-date")).toBeInTheDocument();
  });

  // DIV-020 — amount field present (F25 stable id)
  it("renders an amount field with stable id (DIV-020)", () => {
    render(<DividendTransactionModal {...BASE_PROPS} />);
    expect(screen.getByTestId("dividend-trx-amount")).toBeInTheDocument();
  });

  // DIV-020 — note field present (F25 stable id)
  it("renders a note field with stable id (DIV-020)", () => {
    render(<DividendTransactionModal {...BASE_PROPS} />);
    expect(screen.getByTestId("dividend-trx-note")).toBeInTheDocument();
  });

  // DIV-022 — exchange rate field hidden when showExchangeRate false
  it("does NOT render exchange rate field when showExchangeRate is false (DIV-022)", () => {
    mockUseDividendTransaction.mockReturnValue(makeHookReturn({ showExchangeRate: false }));
    render(<DividendTransactionModal {...BASE_PROPS} />);
    expect(screen.queryByTestId("dividend-trx-exchange-rate")).not.toBeInTheDocument();
  });

  // DIV-022 — exchange rate field shown when showExchangeRate true
  it("renders exchange rate field when showExchangeRate is true (DIV-022)", () => {
    mockUseDividendTransaction.mockReturnValue(
      makeHookReturn({
        showExchangeRate: true,
        formData: {
          assetId: "asset-usd-1",
          date: TODAY,
          amount: "50",
          exchangeRate: "1.08",
          note: "",
        },
      }),
    );
    render(<DividendTransactionModal {...BASE_PROPS} />);
    expect(screen.getByTestId("dividend-trx-exchange-rate")).toBeInTheDocument();
  });

  // DIV-021 — submit button disabled when form is invalid (F25 stable form id)
  it("submit button is disabled when isFormValid is false (DIV-021)", () => {
    mockUseDividendTransaction.mockReturnValue(makeHookReturn({ isFormValid: false }));
    render(<DividendTransactionModal {...BASE_PROPS} />);
    const submitButton = screen.getByRole("button", {
      name: /^dividend\.action_record$/i,
    });
    expect(submitButton).toBeDisabled();
  });

  // DIV-021 — submit button enabled when form is valid
  it("submit button is enabled when isFormValid is true (DIV-021)", () => {
    mockUseDividendTransaction.mockReturnValue(makeHookReturn({ isFormValid: true }));
    render(<DividendTransactionModal {...BASE_PROPS} />);
    const submitButton = screen.getByRole("button", {
      name: /^dividend\.action_record$/i,
    });
    expect(submitButton).not.toBeDisabled();
  });

  // DIV-025 — inline error rendered as role="alert" when error is set (F27)
  it("renders an alert with the error message when error is set (DIV-025)", () => {
    mockUseDividendTransaction.mockReturnValue(
      makeHookReturn({ error: { key: "error.AssetNotHeld" } }),
    );
    render(<DividendTransactionModal {...BASE_PROPS} />);
    const alert = screen.getByRole("alert");
    expect(alert).toBeInTheDocument();
    expect(alert).toHaveTextContent("error.AssetNotHeld");
  });

  // DIV-025 — no alert when error is null
  it("does not render an alert when error is null (DIV-025)", () => {
    mockUseDividendTransaction.mockReturnValue(makeHookReturn({ error: null }));
    render(<DividendTransactionModal {...BASE_PROPS} />);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  // DIV-025 — submit disabled and spinner shown while isSubmitting
  it("submit button is disabled while isSubmitting is true (DIV-025)", () => {
    mockUseDividendTransaction.mockReturnValue(
      makeHookReturn({ isSubmitting: true, isFormValid: true }),
    );
    render(<DividendTransactionModal {...BASE_PROPS} />);
    const submitButton = screen.getByRole("button", {
      name: /^dividend\.action_record$/i,
    });
    expect(submitButton).toBeDisabled();
  });

  // UI → gateway: selecting an asset in the combobox calls handleChange (F25 stable id)
  it("calls handleChange with assetId when user selects an asset", async () => {
    const handleChange = vi.fn();
    mockUseDividendTransaction.mockReturnValue(makeHookReturn({ handleChange }));
    render(<DividendTransactionModal {...BASE_PROPS} />);

    await userEvent.click(screen.getByTestId("combobox-dividend-trx-asset"));

    expect(handleChange).toHaveBeenCalledWith("assetId", "asset-eur-1");
  });

  // Field onChange wiring: amount / note fire handleChange with their field key.
  it("fires handleChange for the amount and note fields", () => {
    const handleChange = vi.fn();
    mockUseDividendTransaction.mockReturnValue(makeHookReturn({ handleChange }));
    render(<DividendTransactionModal {...BASE_PROPS} />);

    fireEvent.change(screen.getByTestId("dividend-trx-amount"), { target: { value: "12.50" } });
    expect(handleChange).toHaveBeenCalledWith("amount", "12.50");

    fireEvent.change(screen.getByTestId("dividend-trx-note"), { target: { value: "Q2" } });
    expect(handleChange).toHaveBeenCalledWith("note", "Q2");
  });

  // DIV-022 — the exchange-rate field (shown for a foreign-currency asset) fires
  // handleChange, and the amount label reflects the selected asset's currency.
  it("fires handleChange for the exchange-rate field and labels the amount in the asset currency (DIV-022)", () => {
    const handleChange = vi.fn();
    mockUseDividendTransaction.mockReturnValue(
      makeHookReturn({
        handleChange,
        showExchangeRate: true,
        amountCurrency: "USD",
        formData: {
          assetId: "asset-usd-1",
          date: TODAY,
          amount: "50",
          exchangeRate: "1.08",
          note: "",
        },
      }),
    );
    render(<DividendTransactionModal {...BASE_PROPS} />);

    fireEvent.change(screen.getByTestId("dividend-trx-exchange-rate"), {
      target: { value: "1.10" },
    });
    expect(handleChange).toHaveBeenCalledWith("exchangeRate", "1.10");
    // selectedCurrency resolves from the chosen asset (USD), not the account (EUR).
    expect(screen.getByText(/USD/)).toBeInTheDocument();
  });

  // UI → gateway: form submit calls handleSubmit (F25 stable form id)
  it("calls handleSubmit when form is submitted (F25)", () => {
    const handleSubmit = vi.fn((e: React.FormEvent) => e.preventDefault());
    mockUseDividendTransaction.mockReturnValue(makeHookReturn({ isFormValid: true, handleSubmit }));
    const { container } = render(<DividendTransactionModal {...BASE_PROPS} />);

    const form = container.querySelector("#dividend-transaction-form");
    if (!form) throw new Error("expected #dividend-transaction-form to be in the DOM");
    fireEvent.submit(form);

    expect(handleSubmit).toHaveBeenCalled();
  });

  // Cancel button calls onClose
  it("calls onClose when cancel button is clicked", async () => {
    const onClose = vi.fn();
    render(<DividendTransactionModal {...BASE_PROPS} onClose={onClose} />);

    const cancelButton = screen.getByRole("button", {
      name: /action\.cancel/i,
    });
    await userEvent.click(cancelButton);

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  // DIV-028 — the account-currency switch renders for a foreign asset and
  // drives the hook's mode setter.
  it("toggles the account-currency entry mode from its checkbox (DIV-028)", () => {
    const setAmountInAccountCurrency = vi.fn();
    mockUseDividendTransaction.mockReturnValue(
      makeHookReturn({ showCurrencyModeSwitch: true, setAmountInAccountCurrency }),
    );
    render(<DividendTransactionModal {...BASE_PROPS} />);

    fireEvent.click(screen.getByTestId("dividend-trx-account-currency-mode"));
    expect(setAmountInAccountCurrency).toHaveBeenCalledWith(true);
  });

  // DIV-028 — no switch for a same-currency asset.
  it("hides the account-currency switch when the currencies match (DIV-028)", () => {
    mockUseDividendTransaction.mockReturnValue(makeHookReturn({ showCurrencyModeSwitch: false }));
    render(<DividendTransactionModal {...BASE_PROPS} />);

    expect(screen.queryByTestId("dividend-trx-account-currency-mode")).toBeNull();
  });
});
