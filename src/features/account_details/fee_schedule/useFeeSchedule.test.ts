import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { logger } from "@/lib/logger";
import { useFeeSchedule } from "./useFeeSchedule";

// ── Hoisted mocks ──────────────────────────────────────────────────────────────
const {
  mockGetFeeSchedule,
  mockCreateFeeSchedule,
  mockUpdateFeeSchedule,
  mockDeleteFeeSchedule,
  mockShowSnackbar,
} = vi.hoisted(() => ({
  mockGetFeeSchedule: vi.fn(),
  mockCreateFeeSchedule: vi.fn(),
  mockUpdateFeeSchedule: vi.fn(),
  mockDeleteFeeSchedule: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    getFeeSchedule: mockGetFeeSchedule,
    createFeeSchedule: mockCreateFeeSchedule,
    updateFeeSchedule: mockUpdateFeeSchedule,
    deleteFeeSchedule: mockDeleteFeeSchedule,
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
const TODAY = new Date().toISOString().slice(0, 10);

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

const BASE_PROPS = {
  accountId: "account-1",
  assetId: "asset-equity-1",
  onSubmitSuccess: vi.fn(),
};

const EXISTING_SCHEDULE = {
  id: "sched-1",
  account_id: "account-1",
  asset_id: "asset-equity-1",
  annual_rate_percent_micros: 1_500_000, // 1.5%
  frequency: "Monthly" as const,
  start_date: "2025-01-01",
  end_date: null,
  active: true,
  last_applied_period: null,
};

// ── Tests ──────────────────────────────────────────────────────────────────────
describe("useFeeSchedule (FEE-030/060/061/062)", () => {
  beforeEach(() => {
    mockGetFeeSchedule.mockReset();
    mockCreateFeeSchedule.mockReset();
    mockUpdateFeeSchedule.mockReset();
    mockDeleteFeeSchedule.mockReset();
    mockShowSnackbar.mockReset();
    vi.mocked(logger.error).mockClear();
    BASE_PROPS.onSubmitSuccess.mockClear();
  });

  // FEE-030 — calls getFeeSchedule on mount with accountId and assetId
  it("calls getFeeSchedule on mount with accountId and assetId", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});
    expect(mockGetFeeSchedule).toHaveBeenCalledWith("account-1", "asset-equity-1");
  });

  // FEE-030 — no existing schedule: isExisting false, isLoading false after mount
  it("isExisting is false and isLoading is false when no schedule exists", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});
    expect(result.current.isExisting).toBe(false);
    expect(result.current.isLoading).toBe(false);
  });

  // FEE-030 — initial defaults when no schedule exists
  it("form defaults to Monthly / today / empty rate when no schedule exists", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});
    expect(result.current.formData.frequency).toBe("Monthly");
    expect(result.current.formData.startDate).toBe(TODAY);
    expect(result.current.formData.ratePercent).toBe("");
    expect(result.current.formData.endDate).toBe("");
    expect(result.current.formData.active).toBe(true);
  });

  // FEE-060 — existing schedule: isExisting true and form prefilled
  it("isExisting is true and form is prefilled when a schedule exists", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});
    expect(result.current.isExisting).toBe(true);
    expect(result.current.formData.ratePercent).toBe("1.500"); // microToDecimal(1_500_000, 3)
    expect(result.current.formData.frequency).toBe("Monthly");
    expect(result.current.formData.startDate).toBe("2025-01-01");
    expect(result.current.formData.endDate).toBe("");
    expect(result.current.formData.active).toBe(true);
  });

  // FEE-032 — form invalid when ratePercent is empty
  it("isFormValid false when ratePercent is empty", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});
    expect(result.current.isFormValid).toBe(false);
  });

  // FEE-032 — form valid with valid ratePercent and startDate
  it("isFormValid true with a valid ratePercent and startDate", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});
    act(() => result.current.handleChange("ratePercent", "1.5"));
    expect(result.current.isFormValid).toBe(true);
  });

  // FEE-030 — create path: submit calls createFeeSchedule when !isExisting
  it("submit calls createFeeSchedule when no existing schedule (create path)", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    mockCreateFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    act(() => result.current.handleChange("ratePercent", "1.5"));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockCreateFeeSchedule).toHaveBeenCalledWith(
      expect.objectContaining({
        account_id: "account-1",
        asset_id: "asset-equity-1",
        annual_rate_percent_micros: 1_500_000,
        frequency: "Monthly",
        start_date: TODAY,
        end_date: null,
      }),
    );
    expect(mockUpdateFeeSchedule).not.toHaveBeenCalled();
  });

  // FEE-030 — create success: snackbar "fee_schedule.saved" + onSubmitSuccess
  it("shows fee_schedule.saved snackbar and calls onSubmitSuccess on create success", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    mockCreateFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    act(() => result.current.handleChange("ratePercent", "1.5"));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("fee_schedule.saved", "success");
    expect(BASE_PROPS.onSubmitSuccess).toHaveBeenCalled();
  });

  // FEE-060 — update path: submit calls updateFeeSchedule when isExisting
  it("submit calls updateFeeSchedule when an existing schedule is loaded (update path)", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    mockUpdateFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    // Form prefilled from existing schedule — submit as-is
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpdateFeeSchedule).toHaveBeenCalledWith(
      expect.objectContaining({
        account_id: "account-1",
        asset_id: "asset-equity-1",
        annual_rate_percent_micros: 1_500_000,
        end_date: null,
        active: true,
      }),
    );
    expect(mockCreateFeeSchedule).not.toHaveBeenCalled();
  });

  // FEE-060 — update success: snackbar "fee_schedule.saved" + onSubmitSuccess
  it("shows fee_schedule.saved snackbar and calls onSubmitSuccess on update success", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    mockUpdateFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("fee_schedule.saved", "success");
    expect(BASE_PROPS.onSubmitSuccess).toHaveBeenCalled();
  });

  // FEE-062 — delete path: handleDelete calls deleteFeeSchedule
  it("handleDelete calls deleteFeeSchedule with accountId and assetId", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    mockDeleteFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    await act(async () => {
      await result.current.handleDelete();
    });

    expect(mockDeleteFeeSchedule).toHaveBeenCalledWith("account-1", "asset-equity-1");
  });

  // FEE-062 — delete success: snackbar "fee_schedule.deleted" + onSubmitSuccess
  it("shows fee_schedule.deleted snackbar and calls onSubmitSuccess on delete success", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    mockDeleteFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    await act(async () => {
      await result.current.handleDelete();
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("fee_schedule.deleted", "success");
    expect(BASE_PROPS.onSubmitSuccess).toHaveBeenCalled();
  });

  // Error on create: sets inline error (F27)
  it("surfaces gateway error as inline error on create failure (F27)", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    mockCreateFeeSchedule.mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    act(() => result.current.handleChange("ratePercent", "1.5"));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(mockShowSnackbar).not.toHaveBeenCalled();
  });

  // Error on update: logged and sets inline error (F27)
  it("logs and surfaces gateway error as inline error on update failure (F27)", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    mockUpdateFeeSchedule.mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(logger.error).toHaveBeenCalledWith(
      "[useFeeSchedule] save failed",
      expect.objectContaining({ error: { code: "DatabaseError" } }),
    );
    expect(mockShowSnackbar).not.toHaveBeenCalled();
  });

  // Error on delete: sets inline error (F27)
  it("surfaces gateway error as inline error on delete failure (F27)", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    mockDeleteFeeSchedule.mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    await act(async () => {
      await result.current.handleDelete();
    });

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(logger.error).toHaveBeenCalledWith(
      "[useFeeSchedule] delete failed",
      expect.objectContaining({ error: { code: "DatabaseError" } }),
    );
    expect(mockShowSnackbar).not.toHaveBeenCalled();
  });

  // Error on getFeeSchedule: sets inline error
  it("sets inline error and logs when getFeeSchedule fails", async () => {
    mockGetFeeSchedule.mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(logger.error).toHaveBeenCalledWith(
      "[useFeeSchedule] getFeeSchedule failed",
      expect.objectContaining({ error: { code: "DatabaseError" } }),
    );
  });

  // FEE-032 — handleSubmit blocked when ratePercent is invalid (no gateway call)
  it("handleSubmit does not call gateway when ratePercent is invalid", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});
    // ratePercent is "" — invalid, submit re-validates inline

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockCreateFeeSchedule).not.toHaveBeenCalled();
    expect(mockUpdateFeeSchedule).not.toHaveBeenCalled();
  });

  // FEE-032 — validation error set when ratePercent is invalid at submit
  it("sets validation error on submit when ratePercent is empty", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "validation.percentage_not_positive" });
  });

  // isSubmitting toggles during submit
  it("isSubmitting is true during submit and false after", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    let resolvePromise!: (value: unknown) => void;
    mockCreateFeeSchedule.mockReturnValue(
      new Promise((resolve) => {
        resolvePromise = resolve;
      }),
    );
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    act(() => result.current.handleChange("ratePercent", "1.5"));

    act(() => {
      void result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.isSubmitting).toBe(true);

    await act(async () => {
      resolvePromise({ status: "ok", data: EXISTING_SCHEDULE });
    });

    expect(result.current.isSubmitting).toBe(false);
  });

  // isSubmitting toggles during delete
  it("isSubmitting is true during delete and false after", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    let resolvePromise!: (value: unknown) => void;
    mockDeleteFeeSchedule.mockReturnValue(
      new Promise((resolve) => {
        resolvePromise = resolve;
      }),
    );
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    act(() => {
      void result.current.handleDelete();
    });

    expect(result.current.isSubmitting).toBe(true);

    await act(async () => {
      resolvePromise({ status: "ok", data: null });
    });

    expect(result.current.isSubmitting).toBe(false);
  });

  // FEE-045 — endDate is passed to createFeeSchedule when set
  it("passes end_date to createFeeSchedule when endDate is set", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: null });
    mockCreateFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    act(() => {
      result.current.handleChange("ratePercent", "1.5");
      result.current.handleChange("startDate", "2024-01-01");
      result.current.handleChange("endDate", "2024-12-31");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockCreateFeeSchedule).toHaveBeenCalledWith(
      expect.objectContaining({ end_date: "2024-12-31" }),
    );
  });

  // FEE-061 — active flag is sent to updateFeeSchedule
  it("passes active flag to updateFeeSchedule when toggled (FEE-061)", async () => {
    mockGetFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    mockUpdateFeeSchedule.mockResolvedValue({ status: "ok", data: EXISTING_SCHEDULE });
    const { result } = renderHook(() => useFeeSchedule(BASE_PROPS));
    await act(async () => {});

    act(() => result.current.handleChange("active", false));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpdateFeeSchedule).toHaveBeenCalledWith(expect.objectContaining({ active: false }));
  });
});
