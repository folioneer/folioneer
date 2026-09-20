import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ManagementFeeModal } from "./ManagementFeeModal";

// ── Mock the hook that drives the component ────────────────────────────────────
const { mockUseManagementFee } = vi.hoisted(() => ({
  mockUseManagementFee: vi.fn(),
}));

vi.mock("./useManagementFee", () => ({
  useManagementFee: (...args: unknown[]) => mockUseManagementFee(...args),
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
    mode: "percent",
    percent: "",
    resultingQuantity: "",
    note: "",
  },
  error: null,
  preview: null,
  previewError: null,
  isSubmitting: false,
  isFormValid: false,
  handleChange: vi.fn(),
  handleModeChange: vi.fn(),
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
describe("ManagementFeeModal (FEE-020/021/022)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseManagementFee.mockReturnValue(makeHookReturn());
  });

  // FEE-020 — the two entry modes, percentage by default (F25 stable ids)
  it("renders the mode toggle with percentage selected and no quantity field (FEE-020)", () => {
    render(<ManagementFeeModal {...BASE_PROPS} />);
    expect(document.getElementById("management-fee-mode-percent")).toHaveAttribute(
      "aria-checked",
      "true",
    );
    expect(document.getElementById("management-fee-mode-quantity")).toHaveAttribute(
      "aria-checked",
      "false",
    );
    expect(document.getElementById("management-fee-resulting-quantity")).toBeNull();
  });

  it("asks the hook to switch mode when the quantity toggle is clicked (FEE-020)", async () => {
    const handleModeChange = vi.fn();
    mockUseManagementFee.mockReturnValue(makeHookReturn({ handleModeChange }));
    render(<ManagementFeeModal {...BASE_PROPS} />);
    await userEvent.click(document.getElementById("management-fee-mode-quantity") as HTMLElement);
    expect(handleModeChange).toHaveBeenCalledWith("quantity");
  });

  // FEE-029 — quantity mode shows the field and the backend's preview, not the percentage
  it("shows the resulting-quantity field and the removal preview in quantity mode (FEE-029)", () => {
    mockUseManagementFee.mockReturnValue(
      makeHookReturn({
        formData: {
          assetId: "asset-equity-1",
          date: "2026-09-19",
          mode: "quantity",
          percent: "",
          resultingQuantity: "24.5",
          note: "",
        },
        preview: {
          held_quantity: 25_000_000,
          removed_quantity: 500_000,
          percent_micros: 2_000_000,
        },
      }),
    );
    render(<ManagementFeeModal {...BASE_PROPS} />);
    expect(document.getElementById("management-fee-resulting-quantity")).not.toBeNull();
    expect(document.getElementById("management-fee-percent")).toBeNull();
    expect(document.getElementById("management-fee-preview")).toHaveTextContent(
      "management_fee.preview",
    );
  });

  it("shows a rejected preview as the field's error and no preview line (FEE-029)", () => {
    mockUseManagementFee.mockReturnValue(
      makeHookReturn({
        formData: {
          assetId: "asset-equity-1",
          date: "2026-09-19",
          mode: "quantity",
          percent: "",
          resultingQuantity: "26",
          note: "",
        },
        previewError: { key: "error.ResultingQuantityNotBelowHeld", vars: { held: "25" } },
      }),
    );
    render(<ManagementFeeModal {...BASE_PROPS} />);
    expect(screen.getByText("error.ResultingQuantityNotBelowHeld")).toBeInTheDocument();
    expect(document.getElementById("management-fee-preview")).toBeNull();
  });

  // FEE-011/012 — asset selector renders all held assets (F25 stable id)
  it("renders the asset selector with all held assets (FEE-011/012, F25)", () => {
    render(<ManagementFeeModal {...BASE_PROPS} />);
    expect(screen.getByTestId("management-fee-asset-select")).toBeInTheDocument();
    expect(screen.getByText("Apple Inc")).toBeInTheDocument();
    expect(screen.getByText("Tesla Inc")).toBeInTheDocument();
  });

  // FEE-021 — date field present with stable id (F25)
  it("renders a date field with stable id (FEE-021, F25)", () => {
    render(<ManagementFeeModal {...BASE_PROPS} />);
    expect(screen.getByTestId("management-fee-date")).toBeInTheDocument();
  });

  // FEE-021 — percent field present with stable id (F25)
  it("renders a percent field with stable id (FEE-021, F25)", () => {
    render(<ManagementFeeModal {...BASE_PROPS} />);
    expect(screen.getByTestId("management-fee-percent")).toBeInTheDocument();
  });

  // Note field present with stable id (F25)
  it("renders a note field with stable id (F25)", () => {
    render(<ManagementFeeModal {...BASE_PROPS} />);
    expect(screen.getByTestId("management-fee-note")).toBeInTheDocument();
  });

  // FEE-021 — submit button disabled when form is invalid (F25)
  it("submit button is disabled when isFormValid is false (FEE-021, F25)", () => {
    mockUseManagementFee.mockReturnValue(makeHookReturn({ isFormValid: false }));
    render(<ManagementFeeModal {...BASE_PROPS} />);
    expect(screen.getByTestId("management-fee-submit")).toBeDisabled();
  });

  // FEE-021 — submit button enabled when form is valid
  it("submit button is enabled when isFormValid is true (FEE-021)", () => {
    mockUseManagementFee.mockReturnValue(makeHookReturn({ isFormValid: true }));
    render(<ManagementFeeModal {...BASE_PROPS} />);
    expect(screen.getByTestId("management-fee-submit")).not.toBeDisabled();
  });

  // FEE-025 — inline error rendered as role="alert" when error is set (F27)
  it("renders an alert with the error i18n key when error is set (FEE-025, F27)", () => {
    mockUseManagementFee.mockReturnValue(makeHookReturn({ error: { key: "error.AssetNotHeld" } }));
    render(<ManagementFeeModal {...BASE_PROPS} />);
    const alert = screen.getByRole("alert");
    expect(alert).toBeInTheDocument();
    expect(alert).toHaveTextContent("error.AssetNotHeld");
  });

  // FEE-025 — no alert when error is null
  it("does not render an alert when error is null (FEE-025)", () => {
    mockUseManagementFee.mockReturnValue(makeHookReturn({ error: null }));
    render(<ManagementFeeModal {...BASE_PROPS} />);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  // FEE-025 — submit disabled while isSubmitting
  it("submit button is disabled while isSubmitting is true (FEE-025)", () => {
    mockUseManagementFee.mockReturnValue(makeHookReturn({ isSubmitting: true, isFormValid: true }));
    render(<ManagementFeeModal {...BASE_PROPS} />);
    expect(screen.getByTestId("management-fee-submit")).toBeDisabled();
  });

  // UI → hook: selecting an asset calls handleChange("assetId", ...) (F25)
  it("calls handleChange with assetId when user selects an asset (F25)", async () => {
    const handleChange = vi.fn();
    mockUseManagementFee.mockReturnValue(makeHookReturn({ handleChange }));
    render(<ManagementFeeModal {...BASE_PROPS} />);

    const select = screen.getByTestId("management-fee-asset-select");
    await userEvent.selectOptions(select, "asset-equity-1");

    expect(handleChange).toHaveBeenCalledWith("assetId", "asset-equity-1");
  });

  // UI → hook: percent field fires handleChange
  it("fires handleChange for the percent field", () => {
    const handleChange = vi.fn();
    mockUseManagementFee.mockReturnValue(makeHookReturn({ handleChange }));
    render(<ManagementFeeModal {...BASE_PROPS} />);

    fireEvent.change(screen.getByTestId("management-fee-percent"), {
      target: { value: "1.5" },
    });
    expect(handleChange).toHaveBeenCalledWith("percent", "1.5");
  });

  // UI → hook: note field fires handleChange
  it("fires handleChange for the note field", () => {
    const handleChange = vi.fn();
    mockUseManagementFee.mockReturnValue(makeHookReturn({ handleChange }));
    render(<ManagementFeeModal {...BASE_PROPS} />);

    fireEvent.change(screen.getByTestId("management-fee-note"), {
      target: { value: "Q2 management fee" },
    });
    expect(handleChange).toHaveBeenCalledWith("note", "Q2 management fee");
  });

  // Form submit calls handleSubmit (F25 stable form id)
  it("calls handleSubmit when form is submitted (F25)", () => {
    const handleSubmit = vi.fn((e: React.FormEvent) => e.preventDefault());
    mockUseManagementFee.mockReturnValue(makeHookReturn({ isFormValid: true, handleSubmit }));
    const { container } = render(<ManagementFeeModal {...BASE_PROPS} />);

    const form = container.querySelector("#management-fee-form");
    if (!form) throw new Error("expected #management-fee-form to be in the DOM");
    fireEvent.submit(form);

    expect(handleSubmit).toHaveBeenCalled();
  });

  // Cancel button calls onClose
  it("calls onClose when cancel button is clicked", async () => {
    const onClose = vi.fn();
    render(<ManagementFeeModal {...BASE_PROPS} onClose={onClose} />);

    const cancelButton = screen.getByRole("button", { name: /action\.cancel/i });
    await userEvent.click(cancelButton);

    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
