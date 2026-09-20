import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { InterestModal } from "./InterestModal";

// ── Mock the hook that drives the component ────────────────────────────────────
const { mockUseInterestTransaction } = vi.hoisted(() => ({
  mockUseInterestTransaction: vi.fn(),
}));

vi.mock("./useInterestTransaction", () => ({
  useInterestTransaction: (...args: unknown[]) => mockUseInterestTransaction(...args),
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

// ── Shared fixtures ────────────────────────────────────────────────────────────
const heldAssets = [
  { assetId: "asset-fund-1", assetName: "Euro Fund", assetCurrency: "EUR" },
  { assetId: "system-cash-acc-1", assetName: "Cash EUR", assetCurrency: "EUR" },
];

const TODAY = new Date().toISOString().slice(0, 10);

const makeHookReturn = (overrides: Record<string, unknown> = {}) => ({
  formData: {
    assetId: "",
    date: TODAY,
    percent: "",
    quantity: "",
    note: "",
  },
  error: null,
  isSubmitting: false,
  isFormValid: false,
  isAssetLocked: false,
  handleChange: vi.fn(),
  handleSubmit: vi.fn(),
  ...overrides,
});

const BASE_PROPS = {
  isOpen: true,
  onClose: vi.fn(),
  accountId: "account-1",
  heldAssets,
  onSubmitSuccess: vi.fn(),
};

const EDIT_MODE = {
  transactionId: "tx-int-1",
  lockedAssetId: "asset-fund-1",
  lockedAssetName: "Euro Fund",
};

// ── Tests ──────────────────────────────────────────────────────────────────────
describe("InterestModal (INT-020/021/025)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseInterestTransaction.mockReturnValue(makeHookReturn());
  });

  // INT-020 — asset selector renders all eligible holdings, cash line included (F25 stable id)
  it("renders the asset selector with held assets including the cash line (INT-020/023)", () => {
    render(<InterestModal {...BASE_PROPS} />);
    expect(screen.getByTestId("interest-asset")).toBeInTheDocument();
    expect(screen.getByText("Euro Fund")).toBeInTheDocument();
    expect(screen.getByText("Cash EUR")).toBeInTheDocument();
  });

  // INT-020 — date field present with stable id (F25)
  it("renders a date field with stable id (INT-020)", () => {
    render(<InterestModal {...BASE_PROPS} />);
    expect(screen.getByTestId("interest-date")).toBeInTheDocument();
  });

  // INT-020 — percent field present with stable id (F25)
  it("renders a percent field with stable id (INT-020)", () => {
    render(<InterestModal {...BASE_PROPS} />);
    expect(screen.getByTestId("interest-percent")).toBeInTheDocument();
  });

  // INT-020 — quantity field present with stable id (F25)
  it("renders a quantity field with stable id (INT-020)", () => {
    render(<InterestModal {...BASE_PROPS} />);
    expect(screen.getByTestId("interest-quantity")).toBeInTheDocument();
  });

  // INT-020 — note field present with stable id (F25)
  it("renders a note field with stable id (INT-020)", () => {
    render(<InterestModal {...BASE_PROPS} />);
    expect(screen.getByTestId("interest-note")).toBeInTheDocument();
  });

  // INT-020 — NO amount field (INT-020: no money inputs)
  it("does NOT render an amount field (INT-020 — no money inputs)", () => {
    render(<InterestModal {...BASE_PROPS} />);
    expect(screen.queryByTestId("interest-amount")).not.toBeInTheDocument();
  });

  // INT-020 — NO exchange-rate field
  it("does NOT render an exchange-rate field (INT-020 — no money inputs)", () => {
    render(<InterestModal {...BASE_PROPS} />);
    expect(screen.queryByTestId("interest-exchange-rate")).not.toBeInTheDocument();
  });

  // INT-020 — NO fees field
  it("does NOT render a fees field (INT-020 — no money inputs)", () => {
    render(<InterestModal {...BASE_PROPS} />);
    expect(screen.queryByTestId("interest-fees")).not.toBeInTheDocument();
  });

  // INT-021 — submit button disabled when form is invalid (F25)
  it("submit button is disabled when isFormValid is false (INT-021)", () => {
    mockUseInterestTransaction.mockReturnValue(makeHookReturn({ isFormValid: false }));
    render(<InterestModal {...BASE_PROPS} />);
    const submitButton = screen.getByTestId("interest-submit");
    expect(submitButton).toBeDisabled();
  });

  // INT-021 — submit button enabled when form is valid
  it("submit button is enabled when isFormValid is true (INT-021)", () => {
    mockUseInterestTransaction.mockReturnValue(makeHookReturn({ isFormValid: true }));
    render(<InterestModal {...BASE_PROPS} />);
    const submitButton = screen.getByTestId("interest-submit");
    expect(submitButton).not.toBeDisabled();
  });

  // INT-025 — inline error rendered as role="alert" when error is set (F27)
  it("renders an alert with the error i18n key when error is set (INT-025, F27)", () => {
    mockUseInterestTransaction.mockReturnValue(
      makeHookReturn({ error: { key: "error.InterestAmountInvalid" } }),
    );
    render(<InterestModal {...BASE_PROPS} />);
    const alert = screen.getByRole("alert");
    expect(alert).toBeInTheDocument();
    expect(alert).toHaveTextContent("error.InterestAmountInvalid");
  });

  // INT-025 — no alert when error is null
  it("does not render an alert when error is null (INT-025)", () => {
    mockUseInterestTransaction.mockReturnValue(makeHookReturn({ error: null }));
    render(<InterestModal {...BASE_PROPS} />);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  // INT-025 — submit disabled while isSubmitting (spinner visible)
  it("submit button is disabled while isSubmitting is true (INT-025)", () => {
    mockUseInterestTransaction.mockReturnValue(
      makeHookReturn({ isSubmitting: true, isFormValid: true }),
    );
    render(<InterestModal {...BASE_PROPS} />);
    const submitButton = screen.getByTestId("interest-submit");
    expect(submitButton).toBeDisabled();
  });

  // UI → hook: selecting an asset calls handleChange (F25)
  it("calls handleChange with assetId when user selects an asset (F25)", async () => {
    const handleChange = vi.fn();
    mockUseInterestTransaction.mockReturnValue(makeHookReturn({ handleChange }));
    render(<InterestModal {...BASE_PROPS} />);

    const select = screen.getByTestId("interest-asset");
    await userEvent.selectOptions(select, "asset-fund-1");

    expect(handleChange).toHaveBeenCalledWith("assetId", "asset-fund-1");
  });

  // UI → hook: percent field fires handleChange
  it("fires handleChange for the percent field", () => {
    const handleChange = vi.fn();
    mockUseInterestTransaction.mockReturnValue(makeHookReturn({ handleChange }));
    render(<InterestModal {...BASE_PROPS} />);

    fireEvent.change(screen.getByTestId("interest-percent"), { target: { value: "2.5" } });
    expect(handleChange).toHaveBeenCalledWith("percent", "2.5");
  });

  // UI → hook: quantity field fires handleChange
  it("fires handleChange for the quantity field", () => {
    const handleChange = vi.fn();
    mockUseInterestTransaction.mockReturnValue(makeHookReturn({ handleChange }));
    render(<InterestModal {...BASE_PROPS} />);

    fireEvent.change(screen.getByTestId("interest-quantity"), { target: { value: "10" } });
    expect(handleChange).toHaveBeenCalledWith("quantity", "10");
  });

  // UI → hook: note field fires handleChange
  it("fires handleChange for the note field", () => {
    const handleChange = vi.fn();
    mockUseInterestTransaction.mockReturnValue(makeHookReturn({ handleChange }));
    render(<InterestModal {...BASE_PROPS} />);

    fireEvent.change(screen.getByTestId("interest-note"), {
      target: { value: "Euro fund 2025 crediting" },
    });
    expect(handleChange).toHaveBeenCalledWith("note", "Euro fund 2025 crediting");
  });

  // INT-040 — edit mode: asset selector is read-only/disabled when asset is locked
  it("asset selector is disabled when isAssetLocked is true (INT-040)", () => {
    mockUseInterestTransaction.mockReturnValue(makeHookReturn({ isAssetLocked: true }));
    render(<InterestModal {...BASE_PROPS} editMode={EDIT_MODE} />);
    const select = screen.getByTestId("interest-asset");
    expect(select).toBeDisabled();
  });

  // INT-040 — edit mode: the percent field is absent (quantity-only correction)
  it("hides the percent field and shows the edit title in edit mode (INT-040)", () => {
    mockUseInterestTransaction.mockReturnValue(makeHookReturn({ isAssetLocked: true }));
    render(<InterestModal {...BASE_PROPS} editMode={EDIT_MODE} />);
    expect(screen.queryByTestId("interest-percent")).not.toBeInTheDocument();
    expect(screen.getByTestId("interest-quantity")).toBeInTheDocument();
    expect(screen.getByText("interest.edit_title")).toBeInTheDocument();
  });

  // Form submit calls handleSubmit (F25 stable form id)
  it("calls handleSubmit when form is submitted (F25)", () => {
    const handleSubmit = vi.fn((e: React.FormEvent) => e.preventDefault());
    mockUseInterestTransaction.mockReturnValue(makeHookReturn({ isFormValid: true, handleSubmit }));
    const { container } = render(<InterestModal {...BASE_PROPS} />);

    const form = container.querySelector("#interest-form");
    if (!form) throw new Error("expected #interest-form to be in the DOM");
    fireEvent.submit(form);

    expect(handleSubmit).toHaveBeenCalled();
  });

  // Cancel button calls onClose
  it("calls onClose when cancel button is clicked", async () => {
    const onClose = vi.fn();
    render(<InterestModal {...BASE_PROPS} onClose={onClose} />);

    const cancelButton = screen.getByRole("button", { name: /action\.cancel/i });
    await userEvent.click(cancelButton);

    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
