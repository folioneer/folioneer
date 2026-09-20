import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FeeScheduleModal } from "./FeeScheduleModal";

// ── Mock the hook that drives the component ────────────────────────────────────
const { mockUseFeeSchedule } = vi.hoisted(() => ({
  mockUseFeeSchedule: vi.fn(),
}));

vi.mock("./useFeeSchedule", () => ({
  useFeeSchedule: (...args: unknown[]) => mockUseFeeSchedule(...args),
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
const TODAY = new Date().toISOString().slice(0, 10);

const makeHookReturn = (overrides: Record<string, unknown> = {}) => ({
  formData: {
    ratePercent: "",
    frequency: "Monthly",
    startDate: TODAY,
    endDate: "",
    active: true,
  },
  isExisting: false,
  isLoading: false,
  error: null,
  isSubmitting: false,
  isFormValid: false,
  handleChange: vi.fn(),
  handleSubmit: vi.fn(),
  handleDelete: vi.fn(),
  ...overrides,
});

const EXISTING_FORM_DATA = {
  ratePercent: "1.500",
  frequency: "Monthly",
  startDate: "2025-01-01",
  endDate: "",
  active: true,
};

const BASE_PROPS = {
  isOpen: true,
  onClose: vi.fn(),
  accountId: "account-1",
  assetId: "asset-equity-1",
  assetName: "Apple Inc",
  onSubmitSuccess: vi.fn(),
};

// ── Tests ──────────────────────────────────────────────────────────────────────
describe("FeeScheduleModal (FEE-030/032/060/061/062)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseFeeSchedule.mockReturnValue(makeHookReturn());
  });

  // FEE-032 — rate field present with stable id (F25)
  it("renders the rate percentage field with stable id (FEE-032, F25)", () => {
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-rate")).toBeInTheDocument();
  });

  // FEE-034 — frequency field present with stable id (F25)
  it("renders the frequency select field with stable id (FEE-034, F25)", () => {
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-frequency")).toBeInTheDocument();
  });

  // FEE-032 — start-date field present with stable id (F25)
  it("renders the start-date field with stable id (FEE-032, F25)", () => {
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-start-date")).toBeInTheDocument();
  });

  // FEE-045 — end-date field present with stable id (F25)
  it("renders the end-date field with stable id (FEE-045, F25)", () => {
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-end-date")).toBeInTheDocument();
  });

  // Submit button disabled when !isFormValid (F25)
  it("submit button is disabled when isFormValid is false (F25)", () => {
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ isFormValid: false }));
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-submit")).toBeDisabled();
  });

  // Submit button enabled when isFormValid and not loading
  it("submit button is enabled when isFormValid is true and isLoading is false", () => {
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ isFormValid: true, isLoading: false }));
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-submit")).not.toBeDisabled();
  });

  // Submit button disabled while isLoading
  it("submit button is disabled while isLoading is true (even when form is valid)", () => {
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ isFormValid: true, isLoading: true }));
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-submit")).toBeDisabled();
  });

  // Submit button disabled while isSubmitting
  it("submit button is disabled while isSubmitting is true", () => {
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ isSubmitting: true, isFormValid: true }));
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-submit")).toBeDisabled();
  });

  // Error alert rendered when error is set (F27)
  it("renders an alert with the error i18n key when error is set (F27)", () => {
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ error: { key: "error.DatabaseError" } }));
    render(<FeeScheduleModal {...BASE_PROPS} />);
    const alert = screen.getByRole("alert");
    expect(alert).toBeInTheDocument();
    expect(alert).toHaveTextContent("error.DatabaseError");
  });

  // No error alert when error is null
  it("does not render an alert when error is null", () => {
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ error: null }));
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  // FEE-062 — delete button NOT rendered when !isExisting
  it("does not render the delete button when no existing schedule (!isExisting)", () => {
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ isExisting: false }));
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.queryByTestId("fee-schedule-delete")).not.toBeInTheDocument();
  });

  // FEE-062 — delete button rendered when isExisting
  it("renders the delete button when an existing schedule is loaded (isExisting)", () => {
    mockUseFeeSchedule.mockReturnValue(
      makeHookReturn({ isExisting: true, formData: EXISTING_FORM_DATA }),
    );
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-delete")).toBeInTheDocument();
  });

  // FEE-060 — frequency disabled when isExisting (immutable after creation)
  it("frequency field is disabled when isExisting is true (FEE-060)", () => {
    mockUseFeeSchedule.mockReturnValue(
      makeHookReturn({ isExisting: true, formData: EXISTING_FORM_DATA }),
    );
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-frequency")).toBeDisabled();
  });

  // FEE-060 — start-date disabled when isExisting (immutable after creation)
  it("start-date field is disabled when isExisting is true (FEE-060)", () => {
    mockUseFeeSchedule.mockReturnValue(
      makeHookReturn({ isExisting: true, formData: EXISTING_FORM_DATA }),
    );
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-start-date")).toBeDisabled();
  });

  // FEE-061 — status field NOT rendered when !isExisting
  it("status field is not rendered when isExisting is false (FEE-061)", () => {
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ isExisting: false }));
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.queryByTestId("fee-schedule-status")).not.toBeInTheDocument();
  });

  // FEE-061 — status field rendered when isExisting
  it("renders the status field when isExisting is true (FEE-061)", () => {
    mockUseFeeSchedule.mockReturnValue(
      makeHookReturn({ isExisting: true, formData: EXISTING_FORM_DATA }),
    );
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-status")).toBeInTheDocument();
  });

  // UI → hook: rate field change calls handleChange("ratePercent", ...)
  it("fires handleChange with ratePercent when rate field changes", () => {
    const handleChange = vi.fn();
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ handleChange }));
    render(<FeeScheduleModal {...BASE_PROPS} />);

    fireEvent.change(screen.getByTestId("fee-schedule-rate"), { target: { value: "1.5" } });
    expect(handleChange).toHaveBeenCalledWith("ratePercent", "1.5");
  });

  // UI → hook: frequency change calls handleChange
  it("fires handleChange with frequency when frequency field changes", async () => {
    const handleChange = vi.fn();
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ handleChange, isExisting: false }));
    render(<FeeScheduleModal {...BASE_PROPS} />);

    await userEvent.selectOptions(screen.getByTestId("fee-schedule-frequency"), "Quarterly");
    expect(handleChange).toHaveBeenCalledWith("frequency", "Quarterly");
  });

  // Form submit calls handleSubmit (F25 stable form id)
  it("calls handleSubmit when form is submitted (F25)", () => {
    const handleSubmit = vi.fn((e: React.FormEvent) => e.preventDefault());
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ isFormValid: true, handleSubmit }));
    const { container } = render(<FeeScheduleModal {...BASE_PROPS} />);

    const form = container.querySelector("#fee-schedule-form");
    if (!form) throw new Error("expected #fee-schedule-form to be in the DOM");
    fireEvent.submit(form);

    expect(handleSubmit).toHaveBeenCalled();
  });

  // FEE-062 — delete button opens a confirmation; handleDelete fires only on confirm
  it("requires confirmation before calling handleDelete (FEE-062)", async () => {
    const handleDelete = vi.fn();
    mockUseFeeSchedule.mockReturnValue(
      makeHookReturn({ isExisting: true, handleDelete, formData: EXISTING_FORM_DATA }),
    );
    render(<FeeScheduleModal {...BASE_PROPS} />);

    // Clicking Delete opens the confirmation dialog — handleDelete not yet called.
    await userEvent.click(screen.getByTestId("fee-schedule-delete"));
    expect(handleDelete).not.toHaveBeenCalled();

    // Confirming in the dialog fires the delete.
    await userEvent.click(document.querySelector("#fee-schedule-delete-confirm")!);
    expect(handleDelete).toHaveBeenCalledTimes(1);
  });

  // Cancel button calls onClose
  it("calls onClose when cancel button is clicked", async () => {
    const onClose = vi.fn();
    render(<FeeScheduleModal {...BASE_PROPS} onClose={onClose} />);

    const cancelButton = screen.getByRole("button", { name: /action\.cancel/i });
    await userEvent.click(cancelButton);

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  // Frequency field enabled when !isExisting (create mode)
  it("frequency field is enabled when isExisting is false (create mode)", () => {
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ isExisting: false }));
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-frequency")).not.toBeDisabled();
  });

  // Start-date field enabled when !isExisting (create mode)
  it("start-date field is enabled when isExisting is false (create mode)", () => {
    mockUseFeeSchedule.mockReturnValue(makeHookReturn({ isExisting: false }));
    render(<FeeScheduleModal {...BASE_PROPS} />);
    expect(screen.getByTestId("fee-schedule-start-date")).not.toBeDisabled();
  });
});
