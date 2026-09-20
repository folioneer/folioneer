import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { HoldingNoteTarget } from "../shared/types";
import { HoldingNoteModal } from "./HoldingNoteModal";

// ── Mock the hook that drives the component ────────────────────────────────────
const { mockUseHoldingNote } = vi.hoisted(() => ({
  mockUseHoldingNote: vi.fn(),
}));

vi.mock("./useHoldingNote", async (importOriginal) => ({
  // Keep NOTE_TEXT_MAX_LENGTH real; only the hook is stubbed.
  ...(await importOriginal<typeof import("./useHoldingNote")>()),
  useHoldingNote: (...args: unknown[]) => mockUseHoldingNote(...args),
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
const createTarget: HoldingNoteTarget = {
  assetId: "asset-equity-1",
  assetName: "Air Liquide",
  assetCurrency: "EUR",
  existing: null,
};

const editTarget: HoldingNoteTarget = {
  ...createTarget,
  existing: {
    text: "buy 7 shares below 150",
    thresholdPrice: 150_000_000,
    thresholdDirection: "Below",
  },
};

const makeHookReturn = (overrides: Record<string, unknown> = {}) => ({
  formData: {
    text: "",
    alarmEnabled: false,
    direction: "Below",
    price: "",
  },
  isEditMode: false,
  error: null,
  isSubmitting: false,
  isFormValid: false,
  handleChange: vi.fn(),
  handleSubmit: vi.fn(),
  handleDelete: vi.fn(),
  ...overrides,
});

const BASE_PROPS = {
  isOpen: true,
  onClose: vi.fn(),
  accountId: "account-1",
  target: createTarget,
  onSubmitSuccess: vi.fn(),
};

// ── Tests ──────────────────────────────────────────────────────────────────────
describe("HoldingNoteModal (HNO-042)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseHoldingNote.mockReturnValue(makeHookReturn());
  });

  // HNO-042 — textarea + alarm toggle render; alarm fields hidden while off
  it("renders the textarea and the alarm toggle, hiding the alarm fields while off", () => {
    render(<HoldingNoteModal {...BASE_PROPS} />);
    expect(screen.getByTestId("holding-note-text")).toBeInTheDocument();
    expect(screen.getByTestId("holding-note-alarm-toggle")).toBeInTheDocument();
    expect(screen.queryByTestId("holding-note-direction")).not.toBeInTheDocument();
    expect(screen.queryByTestId("holding-note-price")).not.toBeInTheDocument();
  });

  // HNO-011 — the textarea caps input at 500 characters
  it("caps the textarea at 500 characters via maxLength", () => {
    render(<HoldingNoteModal {...BASE_PROPS} />);
    expect(screen.getByTestId("holding-note-text")).toHaveAttribute("maxlength", "500");
  });

  // HNO-042 — toggling the alarm reveals the direction + threshold fields
  it("reveals the direction and price fields when the alarm toggle is on", () => {
    mockUseHoldingNote.mockReturnValue(
      makeHookReturn({
        formData: { text: "note", alarmEnabled: true, direction: "Below", price: "" },
      }),
    );
    render(<HoldingNoteModal {...BASE_PROPS} />);
    expect(screen.getByTestId("holding-note-direction")).toBeInTheDocument();
    expect(screen.getByTestId("holding-note-price")).toBeInTheDocument();
  });

  it("routes the alarm toggle through handleChange", async () => {
    const handleChange = vi.fn();
    mockUseHoldingNote.mockReturnValue(makeHookReturn({ handleChange }));
    render(<HoldingNoteModal {...BASE_PROPS} />);
    await userEvent.click(screen.getByTestId("holding-note-alarm-toggle"));
    expect(handleChange).toHaveBeenCalledWith("alarmEnabled", true);
  });

  it("routes textarea edits through handleChange", () => {
    const handleChange = vi.fn();
    mockUseHoldingNote.mockReturnValue(makeHookReturn({ handleChange }));
    render(<HoldingNoteModal {...BASE_PROPS} />);
    fireEvent.change(screen.getByTestId("holding-note-text"), {
      target: { value: "watch earnings" },
    });
    expect(handleChange).toHaveBeenCalledWith("text", "watch earnings");
  });

  // HNO-011 — save disabled while the form is invalid; enabled when valid
  it("disables the save button while the form is invalid", () => {
    render(<HoldingNoteModal {...BASE_PROPS} />);
    expect(screen.getByTestId("holding-note-submit")).toBeDisabled();
  });

  it("enables the save button when the form is valid", () => {
    mockUseHoldingNote.mockReturnValue(makeHookReturn({ isFormValid: true }));
    render(<HoldingNoteModal {...BASE_PROPS} />);
    expect(screen.getByTestId("holding-note-submit")).not.toBeDisabled();
  });

  // HNO-021/042 — delete only offered when a note already exists
  it("hides the delete button in create mode", () => {
    render(<HoldingNoteModal {...BASE_PROPS} />);
    expect(screen.queryByTestId("holding-note-delete")).not.toBeInTheDocument();
  });

  it("delete goes through the confirm step before calling handleDelete (HNO-021)", async () => {
    const handleDelete = vi.fn();
    mockUseHoldingNote.mockReturnValue(makeHookReturn({ isEditMode: true, handleDelete }));
    render(<HoldingNoteModal {...BASE_PROPS} target={editTarget} />);
    const deleteButton = screen.getByTestId("holding-note-delete");
    expect(deleteButton).toBeInTheDocument();
    await userEvent.click(deleteButton);
    // Not yet — the confirmation dialog intercepts the destructive action.
    expect(handleDelete).not.toHaveBeenCalled();
    expect(screen.getByText("holding_note.delete_confirm_title")).toBeInTheDocument();
    await userEvent.click(screen.getByText("action.delete"));
    expect(handleDelete).toHaveBeenCalledTimes(1);
  });

  // Form submit routes through the hook (E3 stable form id)
  it("calls handleSubmit when the form is submitted", () => {
    const handleSubmit = vi.fn((e: React.FormEvent) => e.preventDefault());
    mockUseHoldingNote.mockReturnValue(makeHookReturn({ handleSubmit }));
    const { container } = render(<HoldingNoteModal {...BASE_PROPS} />);

    const form = container.querySelector("#holding-note-form");
    if (!form) throw new Error("expected #holding-note-form to be in the DOM");
    fireEvent.submit(form);

    expect(handleSubmit).toHaveBeenCalled();
  });

  // F27 — backend rejection rendered inline as role="alert"
  it("renders the submit error as an alert when set", () => {
    mockUseHoldingNote.mockReturnValue(makeHookReturn({ error: { key: "error.NoteTextEmpty" } }));
    render(<HoldingNoteModal {...BASE_PROPS} />);
    expect(screen.getByRole("alert")).toHaveTextContent("error.NoteTextEmpty");
  });

  // Cancel button calls onClose
  it("calls onClose when the cancel button is clicked", async () => {
    const onClose = vi.fn();
    render(<HoldingNoteModal {...BASE_PROPS} onClose={onClose} />);
    await userEvent.click(screen.getByRole("button", { name: /action\.cancel/i }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
