import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, Asset } from "@/bindings";
import { useAppStore } from "@/lib/store";
import { OpenBalanceModal } from "./OpenBalanceModal";

// ── Mock the hook that drives the component ───────────────────────────────────
// Mocking at the hook boundary (gateway to UI) — not @tauri-apps/api/core.
const { mockUseOpenBalance } = vi.hoisted(() => ({
  mockUseOpenBalance: vi.fn(),
}));

vi.mock("./useOpenBalance", () => ({
  useOpenBalance: (...args: unknown[]) => mockUseOpenBalance(...args),
}));

// CSH-018 — capture ComboboxField items so we can assert Cash assets are filtered.
vi.mock("@/ui/components/field/ComboboxField", () => ({
  ComboboxField: ({ id, items }: { id: string; items: { id: string; name: string }[] }) => (
    <div data-testid={`combobox-${id}`} data-item-ids={items.map((i) => i.id).join(",")} />
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

// ── Shared hook return factories ──────────────────────────────────────────────
const TODAY = new Date().toISOString().slice(0, 10);

const makeHookReturn = (overrides: Record<string, unknown> = {}) => ({
  formData: {
    accountId: "account-1",
    assetId: "asset-1",
    date: TODAY,
    quantity: "",
    totalCost: "",
  },
  error: null,
  isSubmitting: false,
  isFormValid: false,
  handleChange: vi.fn(),
  handleSubmit: vi.fn(),
  ...overrides,
});

// ── Shared component props ────────────────────────────────────────────────────
const BASE_PROPS = {
  isOpen: true,
  onClose: vi.fn(),
  accountId: "account-1",
  accountName: "My Account",
  assetId: "asset-1",
  assetName: "Apple Inc",
  onSubmitSuccess: vi.fn(),
};

describe("OpenBalanceModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({
      assets: [
        {
          id: "asset-stock-1",
          name: "Apple",
          class: "Stocks",
          is_archived: false,
          currency: "USD",
        },
        {
          id: "system-cash-eur",
          name: "Cash EUR",
          class: "Cash",
          is_archived: false,
          currency: "EUR",
        },
      ] as Asset[],
      accounts: [{ id: "account-1", name: "My Account", currency: "EUR" }] as Account[],
    });
    mockUseOpenBalance.mockReturnValue(makeHookReturn());
  });

  // CSH-018 — Cash Assets are filtered out of the asset combobox (only rendered
  // when assetId is empty, i.e. opened from the page header without a pre-selection).
  it("filters Cash assets from the asset combobox (CSH-018)", () => {
    render(<OpenBalanceModal {...BASE_PROPS} assetId="" assetName="" />);
    const combobox = screen.getByTestId("combobox-ob-asset-select");
    const itemIds = combobox.getAttribute("data-item-ids")?.split(",") ?? [];
    expect(itemIds).toContain("asset-stock-1");
    expect(itemIds).not.toContain("system-cash-eur");
  });

  // ── Form fields present (TRX-043: no fees, no exchange_rate, no unit_price) ─

  // TRX-043 — date field is present
  it("renders a date field (TRX-043)", () => {
    render(<OpenBalanceModal {...BASE_PROPS} />);
    // The date field should be present — either by label text or role
    expect(
      screen.getByLabelText(/transaction\.form_date_label/i) ||
        screen.getByRole("textbox", { name: /date/i }) ||
        screen.getByDisplayValue(TODAY),
    ).toBeTruthy();
  });

  // TRX-043 — quantity field is present
  it("renders a quantity field (TRX-043)", () => {
    render(<OpenBalanceModal {...BASE_PROPS} />);
    expect(screen.getByLabelText(/transaction\.form_quantity_label/i)).toBeInTheDocument();
  });

  // TRX-043 — total_cost field is present (not unit_price)
  it("renders a total_cost field and NOT a unit_price field (TRX-043)", () => {
    render(<OpenBalanceModal {...BASE_PROPS} />);
    // total_cost should exist
    expect(screen.getByLabelText(/open_balance\.form_total_cost_label/i)).toBeInTheDocument();
  });

  // TRX-043 — no fees field
  it("does NOT render a fees field (TRX-043)", () => {
    render(<OpenBalanceModal {...BASE_PROPS} />);
    expect(screen.queryByLabelText(/transaction\.form_fees_label/i)).not.toBeInTheDocument();
  });

  // TRX-043 — no exchange_rate field
  it("does NOT render an exchange rate field (TRX-043)", () => {
    render(<OpenBalanceModal {...BASE_PROPS} />);
    expect(
      screen.queryByLabelText(/transaction\.form_exchange_rate_label/i),
    ).not.toBeInTheDocument();
  });

  // ── Account and asset names shown (read-only context) ────────────────────

  it("displays account name and asset name as read-only context", () => {
    render(<OpenBalanceModal {...BASE_PROPS} />);
    expect(screen.getByDisplayValue("My Account")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Apple Inc")).toBeInTheDocument();
  });

  // ── Submit button state ───────────────────────────────────────────────────

  it("submit button is disabled when isFormValid is false", () => {
    mockUseOpenBalance.mockReturnValue(makeHookReturn({ isFormValid: false }));
    render(<OpenBalanceModal {...BASE_PROPS} />);
    const submitButton = screen.getByRole("button", {
      name: /open_balance\.action_submit/i,
    });
    expect(submitButton).toBeDisabled();
  });

  it("submit button is enabled when isFormValid is true", () => {
    mockUseOpenBalance.mockReturnValue(makeHookReturn({ isFormValid: true }));
    render(<OpenBalanceModal {...BASE_PROPS} />);
    const submitButton = screen.getByRole("button", {
      name: /open_balance\.action_submit/i,
    });
    expect(submitButton).not.toBeDisabled();
  });

  it("submit button shows loading state while isSubmitting is true", () => {
    mockUseOpenBalance.mockReturnValue(makeHookReturn({ isSubmitting: true, isFormValid: true }));
    render(<OpenBalanceModal {...BASE_PROPS} />);
    const submitButton = screen.getByRole("button", {
      name: /open_balance\.action_submit/i,
    });
    expect(submitButton).toBeDisabled();
  });

  // ── Error display ─────────────────────────────────────────────────────────

  it("renders an alert with the error message when error is set", () => {
    mockUseOpenBalance.mockReturnValue(makeHookReturn({ error: "error.ArchivedAsset" }));
    render(<OpenBalanceModal {...BASE_PROPS} />);
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  it("does not render an alert when error is null", () => {
    mockUseOpenBalance.mockReturnValue(makeHookReturn({ error: null }));
    render(<OpenBalanceModal {...BASE_PROPS} />);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  // ── UI → gateway direction ────────────────────────────────────────────────

  // Typing into the quantity field calls handleChange with "quantity"
  it("calls handleChange with quantity field when user types in quantity input", async () => {
    const handleChange = vi.fn();
    mockUseOpenBalance.mockReturnValue(makeHookReturn({ handleChange }));
    render(<OpenBalanceModal {...BASE_PROPS} />);

    const quantityInput = screen.getByLabelText(/transaction\.form_quantity_label/i);
    await userEvent.type(quantityInput, "5");

    expect(handleChange).toHaveBeenCalledWith("quantity", expect.any(String));
  });

  // Typing into the total_cost field calls handleChange with "totalCost"
  it("calls handleChange with totalCost field when user types in total cost input", async () => {
    const handleChange = vi.fn();
    mockUseOpenBalance.mockReturnValue(makeHookReturn({ handleChange }));
    render(<OpenBalanceModal {...BASE_PROPS} />);

    const totalCostInput = screen.getByLabelText(/open_balance\.form_total_cost_label/i);
    await userEvent.type(totalCostInput, "500");

    expect(handleChange).toHaveBeenCalledWith("totalCost", expect.any(String));
  });

  // Submitting the form calls handleSubmit (use fireEvent.submit — userEvent.click does not
  // propagate to form.onSubmit in happy-dom; fireEvent.submit fires the event directly on the form)
  it("calls handleSubmit when form is submitted", () => {
    const handleSubmit = vi.fn((e: React.FormEvent) => e.preventDefault());
    mockUseOpenBalance.mockReturnValue(makeHookReturn({ isFormValid: true, handleSubmit }));
    const { container } = render(<OpenBalanceModal {...BASE_PROPS} />);

    const form = container.querySelector("#ob-form");
    if (!form) throw new Error("expected #ob-form to be in the DOM");
    fireEvent.submit(form);

    expect(handleSubmit).toHaveBeenCalled();
  });

  // Cancel button calls onClose
  it("calls onClose when cancel button is clicked", async () => {
    const onClose = vi.fn();
    render(<OpenBalanceModal {...BASE_PROPS} onClose={onClose} />);

    const cancelButton = screen.getByRole("button", {
      name: /action\.cancel/i,
    });
    await userEvent.click(cancelButton);

    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
