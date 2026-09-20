import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ScheduledFetchSection } from "./ScheduledFetchSection";

// ── Controlled orchestration hook (mocked per task direction, mirrors
// AccountDetailsView.test.tsx's hook-mocking pattern) ──────────────────────
const { mockUseScheduledFetchSection } = vi.hoisted(() => ({
  mockUseScheduledFetchSection: vi.fn(),
}));

vi.mock("./useScheduledFetchSection", () => ({
  useScheduledFetchSection: () => mockUseScheduledFetchSection(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
    i18n: { language: "en" },
  }),
}));

const makeState = (overrides: Record<string, unknown> = {}) => ({
  isLoading: false,
  loadError: null,
  enabled: false,
  triggerTime: "22:15",
  lastRun: null,
  isConfiguring: false,
  configureError: null,
  configure: vi.fn(),
  ...overrides,
});

describe("ScheduledFetchSection — loading and error states (SPF-061)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseScheduledFetchSection.mockReturnValue(makeState());
  });

  it("shows a loading indicator while status is loading, no status line (SPF-061)", () => {
    mockUseScheduledFetchSection.mockReturnValue(makeState({ isLoading: true }));
    render(<ScheduledFetchSection />);

    expect(screen.getByTestId("scheduled-fetch-loading")).toBeInTheDocument();
    expect(screen.queryByTestId("scheduled-fetch-status")).toBeNull();
  });

  it("shows an inline load error when status loading fails, rest of Settings unaffected (SPF-061)", () => {
    mockUseScheduledFetchSection.mockReturnValue(
      makeState({ loadError: { key: "error.scheduled_fetch.DatabaseError" } }),
    );
    render(<ScheduledFetchSection />);

    expect(screen.getByTestId("scheduled-fetch-load-error")).toBeInTheDocument();
  });
});

describe("ScheduledFetchSection — toggle and trigger-time field (SPF-010)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseScheduledFetchSection.mockReturnValue(makeState());
  });

  it("hides the trigger-time field while the toggle is off (SPF-010)", () => {
    render(<ScheduledFetchSection />);
    expect(screen.queryByTestId("scheduled-fetch-time")).toBeNull();
  });

  it("shows the trigger-time field editable when the toggle is on (SPF-010)", () => {
    mockUseScheduledFetchSection.mockReturnValue(
      makeState({ enabled: true, triggerTime: "19:00" }),
    );
    render(<ScheduledFetchSection />);

    const timeField = screen.getByTestId("scheduled-fetch-time");
    expect(timeField).toHaveValue("19:00");
    expect(timeField).not.toBeDisabled();
  });

  it("calls configure with the toggled value and current time when the toggle is clicked", () => {
    const configure = vi.fn();
    mockUseScheduledFetchSection.mockReturnValue(makeState({ configure }));
    render(<ScheduledFetchSection />);

    fireEvent.click(screen.getByTestId("scheduled-fetch-toggle"));

    expect(configure).toHaveBeenCalledWith(true, "22:15");
  });

  it("calls configure with the new time when the trigger-time field changes", () => {
    const configure = vi.fn();
    mockUseScheduledFetchSection.mockReturnValue(
      makeState({ enabled: true, triggerTime: "19:00", configure }),
    );
    render(<ScheduledFetchSection />);

    fireEvent.change(screen.getByTestId("scheduled-fetch-time"), { target: { value: "20:30" } });

    expect(configure).toHaveBeenCalledWith(true, "20:30");
  });
});

describe("ScheduledFetchSection — status line (SPF-052)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the status line for a completed run", () => {
    mockUseScheduledFetchSection.mockReturnValue(
      makeState({
        enabled: true,
        lastRun: {
          executed_at: "2026-07-12T19:00:00Z",
          trigger_date: "2026-07-12",
          outcome: "Succeeded",
          updated_count: 12,
          skipped_count: 2,
        },
      }),
    );
    render(<ScheduledFetchSection />);

    expect(screen.getByTestId("scheduled-fetch-status")).toBeInTheDocument();
  });

  it("renders the no-download-yet status line when lastRun is null", () => {
    mockUseScheduledFetchSection.mockReturnValue(makeState({ enabled: true, lastRun: null }));
    render(<ScheduledFetchSection />);

    expect(screen.getByTestId("scheduled-fetch-status")).toBeInTheDocument();
  });

  it("renders the failed outcome inline — no popup or dialog (SPF-052)", () => {
    mockUseScheduledFetchSection.mockReturnValue(
      makeState({
        enabled: true,
        lastRun: {
          executed_at: "2026-07-12T19:00:00Z",
          trigger_date: "2026-07-12",
          outcome: "Failed",
          updated_count: 0,
          skipped_count: 0,
        },
      }),
    );
    render(<ScheduledFetchSection />);

    expect(screen.getByTestId("scheduled-fetch-status")).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("ScheduledFetchSection — configure in-flight and errors (SPF-013/060)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("disables the toggle and time field while a configure call is in flight (SPF-060)", () => {
    mockUseScheduledFetchSection.mockReturnValue(makeState({ enabled: true, isConfiguring: true }));
    render(<ScheduledFetchSection />);

    expect(screen.getByTestId("scheduled-fetch-toggle")).toBeDisabled();
    expect(screen.getByTestId("scheduled-fetch-time")).toBeDisabled();
  });

  it("shows an inline error when a configure call is rejected, toggle already reverted (SPF-013)", () => {
    mockUseScheduledFetchSection.mockReturnValue(
      makeState({
        configureError: { key: "error.scheduled_fetch.ScheduleRegistrationFailed" },
      }),
    );
    render(<ScheduledFetchSection />);

    expect(screen.getByTestId("scheduled-fetch-configure-error")).toBeInTheDocument();
  });
});
