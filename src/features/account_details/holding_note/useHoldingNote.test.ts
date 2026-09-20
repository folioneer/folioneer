import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { logger } from "@/lib/logger";
import type { HoldingNoteTarget } from "../shared/types";
import { useHoldingNote } from "./useHoldingNote";

// ── Hoisted mocks ──────────────────────────────────────────────────────────────
const { mockUpsertHoldingNote, mockDeleteHoldingNote, mockShowSnackbar } = vi.hoisted(() => ({
  mockUpsertHoldingNote: vi.fn(),
  mockDeleteHoldingNote: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    upsertHoldingNote: mockUpsertHoldingNote,
    deleteHoldingNote: mockDeleteHoldingNote,
  },
}));

vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en" },
  }),
}));

// ── Fixtures ───────────────────────────────────────────────────────────────────
const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

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

const BASE_PROPS = {
  accountId: "account-1",
  target: createTarget,
  onSubmitSuccess: vi.fn(),
};

// ── Tests ──────────────────────────────────────────────────────────────────────
describe("useHoldingNote — create mode (HNO-042)", () => {
  beforeEach(() => {
    mockUpsertHoldingNote.mockReset();
    mockDeleteHoldingNote.mockReset();
    mockShowSnackbar.mockReset();
    vi.mocked(logger.error).mockClear();
    BASE_PROPS.onSubmitSuccess.mockClear();
  });

  // HNO-042 — blank form, alarm off, save disabled on empty text
  it("starts blank with the alarm off and an invalid form", () => {
    const { result } = renderHook(() => useHoldingNote(BASE_PROPS));
    expect(result.current.isEditMode).toBe(false);
    expect(result.current.formData).toEqual({
      text: "",
      alarmEnabled: false,
      direction: "Below",
      price: "",
    });
    expect(result.current.isFormValid).toBe(false);
  });

  // HNO-011 — non-empty text alone (no alarm) makes the form valid
  it("becomes valid once the text is non-empty", () => {
    const { result } = renderHook(() => useHoldingNote(BASE_PROPS));
    act(() => result.current.handleChange("text", "watch the earnings call"));
    expect(result.current.isFormValid).toBe(true);
  });

  // HNO-011 — whitespace-only text stays invalid (trimmed emptiness)
  it("stays invalid for whitespace-only text", () => {
    const { result } = renderHook(() => useHoldingNote(BASE_PROPS));
    act(() => result.current.handleChange("text", "   "));
    expect(result.current.isFormValid).toBe(false);
  });

  // HNO-011 — over-length text disables save even past the maxLength attr
  it("stays invalid when the trimmed text exceeds 500 characters", () => {
    const { result } = renderHook(() => useHoldingNote(BASE_PROPS));
    act(() => result.current.handleChange("text", "x".repeat(501)));
    expect(result.current.isFormValid).toBe(false);
  });

  // HNO-011 — alarm on requires a strictly positive threshold
  it("requires a positive threshold price while the alarm toggle is on", () => {
    const { result } = renderHook(() => useHoldingNote(BASE_PROPS));
    act(() => {
      result.current.handleChange("text", "alert note");
      result.current.handleChange("alarmEnabled", true);
    });
    expect(result.current.isFormValid).toBe(false);

    act(() => result.current.handleChange("price", "0"));
    expect(result.current.isFormValid).toBe(false);

    act(() => result.current.handleChange("price", "150.5"));
    expect(result.current.isFormValid).toBe(true);
  });

  // HNO-020 — DTO shape without an alarm: both alarm fields null
  it("submits a trimmed text with null alarm fields when the toggle is off", async () => {
    mockUpsertHoldingNote.mockResolvedValue({ status: "ok", data: {} });
    const { result } = renderHook(() => useHoldingNote(BASE_PROPS));

    act(() => result.current.handleChange("text", "  plain reminder  "));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpsertHoldingNote).toHaveBeenCalledWith({
      account_id: "account-1",
      asset_id: "asset-equity-1",
      text: "plain reminder",
      threshold_price: null,
      threshold_direction: null,
    });
    expect(mockShowSnackbar).toHaveBeenCalledWith("holding_note.saved", "success");
    expect(BASE_PROPS.onSubmitSuccess).toHaveBeenCalled();
  });

  // HNO-031 — the decimal threshold converts to micros in the DTO
  it("submits the alarm pair with the threshold converted to micros", async () => {
    mockUpsertHoldingNote.mockResolvedValue({ status: "ok", data: {} });
    const { result } = renderHook(() => useHoldingNote(BASE_PROPS));

    act(() => {
      result.current.handleChange("text", "alert note");
      result.current.handleChange("alarmEnabled", true);
      result.current.handleChange("direction", "Above");
      result.current.handleChange("price", "150.5");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpsertHoldingNote).toHaveBeenCalledWith({
      account_id: "account-1",
      asset_id: "asset-equity-1",
      text: "alert note",
      threshold_price: 150_500_000,
      threshold_direction: "Above",
    });
  });

  // HNO-011 — an invalid form never reaches the gateway
  it("does not call the gateway when the form is invalid", async () => {
    const { result } = renderHook(() => useHoldingNote(BASE_PROPS));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpsertHoldingNote).not.toHaveBeenCalled();
    expect(BASE_PROPS.onSubmitSuccess).not.toHaveBeenCalled();
  });

  // F27 — backend rejection surfaces as an inline typed error via the presenter
  it("surfaces a backend error code as an inline error (F27)", async () => {
    mockUpsertHoldingNote.mockResolvedValue({
      status: "error",
      error: { code: "NoteOnUnheldAsset" },
    });
    const { result } = renderHook(() => useHoldingNote(BASE_PROPS));

    act(() => result.current.handleChange("text", "note"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.NoteOnUnheldAsset" });
    expect(mockShowSnackbar).not.toHaveBeenCalled();
    expect(BASE_PROPS.onSubmitSuccess).not.toHaveBeenCalled();
    expect(logger.error).toHaveBeenCalled();
  });
});

describe("useHoldingNote — edit mode (HNO-020/021)", () => {
  const editProps = { ...BASE_PROPS, target: editTarget };

  beforeEach(() => {
    mockUpsertHoldingNote.mockReset();
    mockDeleteHoldingNote.mockReset();
    mockShowSnackbar.mockReset();
    BASE_PROPS.onSubmitSuccess.mockClear();
  });

  // HNO-020 — the form prefills from the stored note (threshold back to decimal)
  it("prefills text, toggle, direction, and decimal threshold from the stored note", () => {
    const { result } = renderHook(() => useHoldingNote(editProps));
    expect(result.current.isEditMode).toBe(true);
    expect(result.current.formData).toEqual({
      text: "buy 7 shares below 150",
      alarmEnabled: true,
      direction: "Below",
      price: "150.000",
    });
    expect(result.current.isFormValid).toBe(true);
  });

  // HNO-020 — a stored note without an alarm prefills with the toggle off
  it("prefills with the alarm off when the stored note has no threshold", () => {
    const noAlarmTarget: HoldingNoteTarget = {
      ...createTarget,
      existing: { text: "plain note", thresholdPrice: null, thresholdDirection: null },
    };
    const props = { ...BASE_PROPS, target: noAlarmTarget };
    const { result } = renderHook(() => useHoldingNote(props));
    expect(result.current.formData).toEqual({
      text: "plain note",
      alarmEnabled: false,
      direction: "Below",
      price: "",
    });
  });

  // HNO-020 — turning the alarm off on an edit clears both alarm fields
  it("submits null alarm fields when the toggle is switched off on an existing alarm", async () => {
    mockUpsertHoldingNote.mockResolvedValue({ status: "ok", data: {} });
    const { result } = renderHook(() => useHoldingNote(editProps));

    act(() => result.current.handleChange("alarmEnabled", false));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpsertHoldingNote).toHaveBeenCalledWith({
      account_id: "account-1",
      asset_id: "asset-equity-1",
      text: "buy 7 shares below 150",
      threshold_price: null,
      threshold_direction: null,
    });
  });

  // HNO-021 — delete routes through the gateway and confirms with a snackbar
  it("deletes the note and reports success", async () => {
    mockDeleteHoldingNote.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useHoldingNote(editProps));

    await act(async () => {
      await result.current.handleDelete();
    });

    expect(mockDeleteHoldingNote).toHaveBeenCalledWith({
      account_id: "account-1",
      asset_id: "asset-equity-1",
    });
    expect(mockUpsertHoldingNote).not.toHaveBeenCalled();
    expect(mockShowSnackbar).toHaveBeenCalledWith("holding_note.deleted", "success");
    expect(BASE_PROPS.onSubmitSuccess).toHaveBeenCalled();
  });

  // F27 — a delete rejection surfaces inline like the save path
  it("surfaces a delete rejection as an inline error (F27)", async () => {
    mockDeleteHoldingNote.mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const { result } = renderHook(() => useHoldingNote(editProps));

    await act(async () => {
      await result.current.handleDelete();
    });

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(mockShowSnackbar).not.toHaveBeenCalled();
    expect(BASE_PROPS.onSubmitSuccess).not.toHaveBeenCalled();
  });
});
