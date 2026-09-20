import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FreeSharesModal } from "./FreeSharesModal";

// ── Mock the hook that drives the component ────────────────────────────────────
const { mockUseFreeSharesTransaction } = vi.hoisted(() => ({
  mockUseFreeSharesTransaction: vi.fn(),
}));

vi.mock("./useFreeSharesTransaction", () => ({
  useFreeSharesTransaction: (...args: unknown[]) => mockUseFreeSharesTransaction(...args),
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
  { assetId: "asset-equity-1", assetName: "Apple Inc", assetCurrency: "EUR" },
  { assetId: "asset-equity-2", assetName: "Tesla Inc", assetCurrency: "USD" },
];

const TODAY = new Date().toISOString().slice(0, 10);

const makeHookReturn = (overrides: Record<string, unknown> = {}) => ({
  formData: {
    assetId: "",
    date: TODAY,
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

// ── Tests ──────────────────────────────────────────────────────────────────────
describe("FreeSharesModal (FSD-020/021/025)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseFreeSharesTransaction.mockReturnValue(makeHookReturn());
  });

  // FSD-020 — asset selector renders all held assets as options (F25 stable id)
  it("renders the asset selector with all held non-cash assets (FSD-020)", () => {
    render(<FreeSharesModal {...BASE_PROPS} />);
    expect(screen.getByTestId("free-shares-asset-select")).toBeInTheDocument();
    expect(screen.getByText("Apple Inc")).toBeInTheDocument();
    expect(screen.getByText("Tesla Inc")).toBeInTheDocument();
  });

  // FSD-020 — date field present with stable id (F25)
  it("renders a date field with stable id (FSD-020)", () => {
    render(<FreeSharesModal {...BASE_PROPS} />);
    expect(screen.getByTestId("free-shares-date")).toBeInTheDocument();
  });

  // FSD-020 — quantity field present with stable id (F25)
  it("renders a quantity field with stable id (FSD-020)", () => {
    render(<FreeSharesModal {...BASE_PROPS} />);
    expect(screen.getByTestId("free-shares-quantity")).toBeInTheDocument();
  });

  // FSD-020 — note field present with stable id (F25)
  it("renders a note field with stable id (FSD-020)", () => {
    render(<FreeSharesModal {...BASE_PROPS} />);
    expect(screen.getByTestId("free-shares-note")).toBeInTheDocument();
  });

  // FSD-020 — NO amount field (FSD-020: no money inputs)
  it("does NOT render an amount field (FSD-020 — no money inputs)", () => {
    render(<FreeSharesModal {...BASE_PROPS} />);
    expect(screen.queryByTestId("free-shares-amount")).not.toBeInTheDocument();
  });

  // FSD-020 — NO exchange-rate field
  it("does NOT render an exchange-rate field (FSD-020 — no money inputs)", () => {
    render(<FreeSharesModal {...BASE_PROPS} />);
    expect(screen.queryByTestId("free-shares-exchange-rate")).not.toBeInTheDocument();
  });

  // FSD-020 — NO fees field
  it("does NOT render a fees field (FSD-020 — no money inputs)", () => {
    render(<FreeSharesModal {...BASE_PROPS} />);
    expect(screen.queryByTestId("free-shares-fees")).not.toBeInTheDocument();
  });

  // FSD-021 — submit button disabled when form is invalid (F25)
  it("submit button is disabled when isFormValid is false (FSD-021)", () => {
    mockUseFreeSharesTransaction.mockReturnValue(makeHookReturn({ isFormValid: false }));
    render(<FreeSharesModal {...BASE_PROPS} />);
    const submitButton = screen.getByTestId("free-shares-submit");
    expect(submitButton).toBeDisabled();
  });

  // FSD-021 — submit button enabled when form is valid
  it("submit button is enabled when isFormValid is true (FSD-021)", () => {
    mockUseFreeSharesTransaction.mockReturnValue(makeHookReturn({ isFormValid: true }));
    render(<FreeSharesModal {...BASE_PROPS} />);
    const submitButton = screen.getByTestId("free-shares-submit");
    expect(submitButton).not.toBeDisabled();
  });

  // FSD-025 — inline error rendered as role="alert" when error is set (F27)
  it("renders an alert with the error i18n key when error is set (FSD-025, F27)", () => {
    mockUseFreeSharesTransaction.mockReturnValue(
      makeHookReturn({ error: { key: "error.AssetNotHeld" } }),
    );
    render(<FreeSharesModal {...BASE_PROPS} />);
    const alert = screen.getByRole("alert");
    expect(alert).toBeInTheDocument();
    expect(alert).toHaveTextContent("error.AssetNotHeld");
  });

  // FSD-025 — no alert when error is null
  it("does not render an alert when error is null (FSD-025)", () => {
    mockUseFreeSharesTransaction.mockReturnValue(makeHookReturn({ error: null }));
    render(<FreeSharesModal {...BASE_PROPS} />);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  // FSD-025 — submit disabled while isSubmitting (spinner visible)
  it("submit button is disabled while isSubmitting is true (FSD-025)", () => {
    mockUseFreeSharesTransaction.mockReturnValue(
      makeHookReturn({ isSubmitting: true, isFormValid: true }),
    );
    render(<FreeSharesModal {...BASE_PROPS} />);
    const submitButton = screen.getByTestId("free-shares-submit");
    expect(submitButton).toBeDisabled();
  });

  // UI → hook: selecting an asset calls handleChange (F25)
  it("calls handleChange with assetId when user selects an asset (F25)", async () => {
    const handleChange = vi.fn();
    mockUseFreeSharesTransaction.mockReturnValue(makeHookReturn({ handleChange }));
    render(<FreeSharesModal {...BASE_PROPS} />);

    const select = screen.getByTestId("free-shares-asset-select");
    await userEvent.selectOptions(select, "asset-equity-1");

    expect(handleChange).toHaveBeenCalledWith("assetId", "asset-equity-1");
  });

  // UI → hook: quantity field fires handleChange
  it("fires handleChange for the quantity field", () => {
    const handleChange = vi.fn();
    mockUseFreeSharesTransaction.mockReturnValue(makeHookReturn({ handleChange }));
    render(<FreeSharesModal {...BASE_PROPS} />);

    fireEvent.change(screen.getByTestId("free-shares-quantity"), { target: { value: "10" } });
    expect(handleChange).toHaveBeenCalledWith("quantity", "10");
  });

  // UI → hook: note field fires handleChange
  it("fires handleChange for the note field", () => {
    const handleChange = vi.fn();
    mockUseFreeSharesTransaction.mockReturnValue(makeHookReturn({ handleChange }));
    render(<FreeSharesModal {...BASE_PROPS} />);

    fireEvent.change(screen.getByTestId("free-shares-note"), {
      target: { value: "Bonus shares Q2" },
    });
    expect(handleChange).toHaveBeenCalledWith("note", "Bonus shares Q2");
  });

  // FSD-040 — edit mode: asset selector is read-only/disabled when asset is locked
  it("asset selector is disabled when isAssetLocked is true (FSD-040)", () => {
    mockUseFreeSharesTransaction.mockReturnValue(makeHookReturn({ isAssetLocked: true }));
    render(<FreeSharesModal {...BASE_PROPS} />);
    const select = screen.getByTestId("free-shares-asset-select");
    expect(select).toBeDisabled();
  });

  // Form submit calls handleSubmit (F25 stable form id)
  it("calls handleSubmit when form is submitted (F25)", () => {
    const handleSubmit = vi.fn((e: React.FormEvent) => e.preventDefault());
    mockUseFreeSharesTransaction.mockReturnValue(
      makeHookReturn({ isFormValid: true, handleSubmit }),
    );
    const { container } = render(<FreeSharesModal {...BASE_PROPS} />);

    const form = container.querySelector("#free-shares-form");
    if (!form) throw new Error("expected #free-shares-form to be in the DOM");
    fireEvent.submit(form);

    expect(handleSubmit).toHaveBeenCalled();
  });

  // Cancel button calls onClose
  it("calls onClose when cancel button is clicked", async () => {
    const onClose = vi.fn();
    render(<FreeSharesModal {...BASE_PROPS} onClose={onClose} />);

    const cancelButton = screen.getByRole("button", { name: /action\.cancel/i });
    await userEvent.click(cancelButton);

    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
