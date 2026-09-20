import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ManagementFeeEditModal } from "./ManagementFeeEditModal";

// ── Mock the hook that drives the component ────────────────────────────────────
const { mockUseManagementFeeEdit } = vi.hoisted(() => ({
  mockUseManagementFeeEdit: vi.fn(),
}));

vi.mock("./useManagementFeeEdit", () => ({
  useManagementFeeEdit: (...args: unknown[]) => mockUseManagementFeeEdit(...args),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() },
}));

// ── Shared fixtures ────────────────────────────────────────────────────────────
const editContext = {
  transactionId: "tx-fee-1",
  lockedAssetName: "Apple Inc",
  initialDate: "2024-06-15",
  initialQuantity: "1.000",
  initialNote: "",
};

const makeHookReturn = (overrides: Record<string, unknown> = {}) => ({
  formData: { date: "2024-06-15", quantity: "1.000", note: "" },
  error: null,
  isSubmitting: false,
  isFormValid: true,
  handleChange: vi.fn(),
  handleSubmit: vi.fn(),
  ...overrides,
});

const BASE_PROPS = {
  isOpen: true,
  onClose: vi.fn(),
  accountId: "account-1",
  editContext,
  onSubmitSuccess: vi.fn(),
};

// ── Tests ──────────────────────────────────────────────────────────────────────
describe("ManagementFeeEditModal (FEE-063)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseManagementFeeEdit.mockReturnValue(makeHookReturn());
  });

  it("renders the date, quantity, and note fields with the edit title", () => {
    render(<ManagementFeeEditModal {...BASE_PROPS} />);
    expect(screen.getByText("management_fee.edit_title")).toBeInTheDocument();
    expect(screen.getByTestId("management-fee-edit-date")).toBeInTheDocument();
    expect(screen.getByTestId("management-fee-edit-quantity")).toBeInTheDocument();
    expect(screen.getByTestId("management-fee-edit-note")).toBeInTheDocument();
  });

  it("locks the charged asset (immutable on edit, FEE-063)", () => {
    render(<ManagementFeeEditModal {...BASE_PROPS} />);
    const assetSelect = screen.getByLabelText("management_fee.form_asset_label");
    expect(assetSelect).toBeDisabled();
  });

  it("disables the save button while the form is invalid", () => {
    mockUseManagementFeeEdit.mockReturnValue(makeHookReturn({ isFormValid: false }));
    render(<ManagementFeeEditModal {...BASE_PROPS} />);
    expect(screen.getByTestId("management-fee-edit-submit")).toBeDisabled();
  });

  it("surfaces a hook error as an alert", () => {
    mockUseManagementFeeEdit.mockReturnValue(
      makeHookReturn({ error: { key: "error.CascadingOversell" } }),
    );
    render(<ManagementFeeEditModal {...BASE_PROPS} />);
    expect(screen.getByRole("alert")).toHaveTextContent("error.CascadingOversell");
  });
});
