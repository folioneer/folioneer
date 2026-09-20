import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SyncSection } from "./SyncSection";

// Controlled orchestration hook (mocked per task direction, mirrors
// ScheduledFetchSection.test.tsx's hook-mocking pattern).
const { mockUseSyncSection } = vi.hoisted(() => ({ mockUseSyncSection: vi.fn() }));

vi.mock("./useSyncSection", () => ({ useSyncSection: () => mockUseSyncSection() }));

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
  paused: false,
  deviceName: null,
  folder: null,
  lastSyncCompletedAt: null,
  roster: [],
  heldBackCount: 0,
  oldestHeldBackSince: null,
  notices: [],
  inconsistentHoldings: [],
  failures: [],
  isSyncing: false,
  actionError: null,
  handleSyncNow: vi.fn(),
  handlePause: vi.fn(),
  handleResume: vi.fn(),
  handleRename: vi.fn(),
  handleChangeFolder: vi.fn(),
  handleBrowseFolder: vi.fn(),
  confirmingLeave: false,
  requestLeave: vi.fn(),
  cancelLeave: vi.fn(),
  confirmLeave: vi.fn(),
  isEnableModalOpen: false,
  openEnableModal: vi.fn(),
  closeEnableModal: vi.fn(),
  isStartOverModalOpen: false,
  openStartOverModal: vi.fn(),
  closeStartOverModal: vi.fn(),
  ...overrides,
});

describe("SyncSection — disabled state (SYN-010/017)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseSyncSection.mockReturnValue(makeState());
  });

  it("shows the honest-positioning copy and the enable action when disabled (SYN-017)", () => {
    render(<SyncSection />);

    expect(screen.getByText("sync.local_copy_note")).toBeInTheDocument();
    expect(screen.getByTestId("sync-enable")).toBeInTheDocument();
  });

  it("does not show enabled-only actions while disabled", () => {
    render(<SyncSection />);

    expect(screen.queryByTestId("sync-now")).toBeNull();
    expect(screen.queryByTestId("sync-leave")).toBeNull();
  });

  it("opens the enable modal when the enable action is clicked", () => {
    const openEnableModal = vi.fn();
    mockUseSyncSection.mockReturnValue(makeState({ openEnableModal }));
    render(<SyncSection />);

    fireEvent.click(screen.getByTestId("sync-enable"));

    expect(openEnableModal).toHaveBeenCalled();
  });
});

describe("SyncSection — enabled state (SYN-061/063/070/072/073/074/082/084)", () => {
  const enabledState = makeState({
    enabled: true,
    deviceName: "Desktop",
    lastSyncCompletedAt: "2026-08-20T10:00:00Z",
    appVersion: "0.43.0",
    roster: [
      {
        deviceId: "device-2",
        deviceName: "Laptop",
        dataFormatVersion: 3,
        appVersion: "0.42.0",
        lastAppliedAt: null,
      },
      {
        deviceId: "device-3",
        deviceName: "Office",
        dataFormatVersion: 3,
        appVersion: null,
        lastAppliedAt: null,
      },
    ],
  });

  beforeEach(() => {
    vi.clearAllMocks();
    mockUseSyncSection.mockReturnValue(enabledState);
  });

  it("shows device name, last sync time and roster rows", () => {
    render(<SyncSection />);

    expect(screen.getByText("Desktop")).toBeInTheDocument();
    expect(screen.getByText("Laptop")).toBeInTheDocument();
  });

  it("SYN-063: the device name element holds the name alone, not the version beside it", () => {
    const { container } = render(<SyncSection />);

    expect(container.querySelector("#sync-status-device-name")?.textContent).toBe("Desktop");
  });

  it("SYN-063: shows the app version of this computer and of each other computer", () => {
    const { container } = render(<SyncSection />);

    expect(container.querySelector("#sync-status-app-version")).toHaveTextContent("0.43.0");
    expect(container.querySelector("#sync-roster-device-2-version")).toHaveTextContent("0.42.0");
  });

  it("SYN-063: says so when another computer does not publish its version", () => {
    const { container } = render(<SyncSection />);

    expect(container.querySelector("#sync-roster-device-3-version")).toHaveTextContent(
      "sync.app_version_unknown",
    );
  });

  it("shows the held-back count when non-zero", () => {
    mockUseSyncSection.mockReturnValue(
      makeState({ enabled: true, heldBackCount: 3, oldestHeldBackSince: "2026-08-18T10:00:00Z" }),
    );
    render(<SyncSection />);

    expect(screen.getByTestId("sync-held-back")).toBeInTheDocument();
  });

  it("shows a failure line for each failure (SYN-034/035/069/084)", () => {
    mockUseSyncSection.mockReturnValue(
      makeState({
        enabled: true,
        failures: [{ UnreadableFiles: { count: 2 } }, "PortfolioReset"],
      }),
    );
    render(<SyncSection />);

    expect(screen.getAllByTestId(/^sync-failure-/)).toHaveLength(2);
  });

  it("renders exactly the pause action (not resume) when not paused", () => {
    render(<SyncSection />);
    expect(screen.getByTestId("sync-pause")).toBeInTheDocument();
    expect(screen.queryByTestId("sync-resume")).toBeNull();
  });

  it("renders exactly the resume action (not pause) when paused", () => {
    mockUseSyncSection.mockReturnValue(makeState({ enabled: true, paused: true }));
    render(<SyncSection />);
    expect(screen.getByTestId("sync-resume")).toBeInTheDocument();
    expect(screen.queryByTestId("sync-pause")).toBeNull();
  });

  it("calls handleSyncNow when Sync now is clicked", () => {
    const handleSyncNow = vi.fn();
    mockUseSyncSection.mockReturnValue(makeState({ enabled: true, handleSyncNow }));
    render(<SyncSection />);

    fireEvent.click(screen.getByTestId("sync-now"));

    expect(handleSyncNow).toHaveBeenCalled();
  });

  it("calls handlePause when Pause is clicked", () => {
    const handlePause = vi.fn();
    mockUseSyncSection.mockReturnValue(makeState({ enabled: true, handlePause }));
    render(<SyncSection />);

    fireEvent.click(screen.getByTestId("sync-pause"));

    expect(handlePause).toHaveBeenCalled();
  });

  it("calls handleResume when Resume is clicked", () => {
    const handleResume = vi.fn();
    mockUseSyncSection.mockReturnValue(makeState({ enabled: true, paused: true, handleResume }));
    render(<SyncSection />);

    fireEvent.click(screen.getByTestId("sync-resume"));

    expect(handleResume).toHaveBeenCalled();
  });

  it("renders the presented error inline when actionError is set (F27)", () => {
    mockUseSyncSection.mockReturnValue(
      makeState({ enabled: true, actionError: { key: "sync.errors.SyncPaused" } }),
    );
    render(<SyncSection />);

    expect(screen.getByText("sync.errors.SyncPaused")).toBeInTheDocument();
  });

  it("requestLeave opens a confirmation before calling leaveSync (SYN-071/082)", () => {
    const requestLeave = vi.fn();
    mockUseSyncSection.mockReturnValue(makeState({ enabled: true, requestLeave }));
    render(<SyncSection />);

    fireEvent.click(screen.getByTestId("sync-leave"));

    expect(requestLeave).toHaveBeenCalled();
  });

  it("shows the confirmation dialog and calls confirmLeave only when confirmed", () => {
    const confirmLeave = vi.fn();
    mockUseSyncSection.mockReturnValue(
      makeState({ enabled: true, confirmingLeave: true, confirmLeave }),
    );
    render(<SyncSection />);

    expect(screen.getByTestId("sync-leave-confirm")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("sync-leave-confirm"));

    expect(confirmLeave).toHaveBeenCalled();
  });

  it("renames through the prompt and closes it once the backend accepted the name (SYN-072)", async () => {
    const handleRename = vi.fn().mockResolvedValue(true);
    mockUseSyncSection.mockReturnValue(
      makeState({ enabled: true, deviceName: "Desktop", handleRename }),
    );
    render(<SyncSection />);

    fireEvent.click(screen.getByTestId("sync-rename"));
    fireEvent.change(screen.getByLabelText("sync.rename_prompt_label"), {
      target: { value: "Laptop" },
    });
    fireEvent.click(screen.getByTestId("sync-prompt-submit"));

    expect(handleRename).toHaveBeenCalledWith("Laptop");
    await waitFor(() => expect(screen.queryByTestId("sync-prompt-submit")).toBeNull());
  });

  it("keeps the prompt open when the backend rejects the new folder (SYN-074, F27)", async () => {
    const handleChangeFolder = vi.fn().mockResolvedValue(false);
    mockUseSyncSection.mockReturnValue(
      makeState({ enabled: true, folder: "/home/user/sync", handleChangeFolder }),
    );
    render(<SyncSection />);

    fireEvent.click(screen.getByTestId("sync-change-folder"));
    fireEvent.click(screen.getByTestId("sync-prompt-submit"));

    expect(handleChangeFolder).toHaveBeenCalledWith("/home/user/sync");
    await waitFor(() => expect(handleChangeFolder).toHaveBeenCalledTimes(1));
    expect(screen.getByTestId("sync-prompt-submit")).toBeInTheDocument();
  });

  it("offers Browse in the change-folder prompt (SYN-074, F25)", () => {
    mockUseSyncSection.mockReturnValue(makeState({ enabled: true, folder: "/home/user/sync" }));
    render(<SyncSection />);

    fireEvent.click(screen.getByTestId("sync-change-folder"));

    expect(screen.getByTestId("sync-prompt-browse")).toBeInTheDocument();
  });

  it("offers no Browse in the rename prompt — it takes a name, not a path (SYN-072)", () => {
    mockUseSyncSection.mockReturnValue(makeState({ enabled: true, deviceName: "Desktop" }));
    render(<SyncSection />);

    fireEvent.click(screen.getByTestId("sync-rename"));

    expect(screen.queryByTestId("sync-prompt-browse")).toBeNull();
  });

  it("puts the picked folder in the field and submits it (SYN-074)", async () => {
    const handleBrowseFolder = vi.fn().mockResolvedValue("/media/phil/KEY/Folioneer");
    const handleChangeFolder = vi.fn().mockResolvedValue(true);
    mockUseSyncSection.mockReturnValue(
      makeState({
        enabled: true,
        folder: "/home/user/sync",
        handleBrowseFolder,
        handleChangeFolder,
      }),
    );
    render(<SyncSection />);

    fireEvent.click(screen.getByTestId("sync-change-folder"));
    fireEvent.click(screen.getByTestId("sync-prompt-browse"));

    await waitFor(() =>
      expect(screen.getByDisplayValue("/media/phil/KEY/Folioneer")).toBeInTheDocument(),
    );

    fireEvent.click(screen.getByTestId("sync-prompt-submit"));

    await waitFor(() =>
      expect(handleChangeFolder).toHaveBeenCalledWith("/media/phil/KEY/Folioneer"),
    );
  });

  it("leaves the typed folder untouched when the picker is cancelled (SYN-074)", async () => {
    const handleBrowseFolder = vi.fn().mockResolvedValue(null);
    mockUseSyncSection.mockReturnValue(
      makeState({ enabled: true, folder: "/home/user/sync", handleBrowseFolder }),
    );
    render(<SyncSection />);

    fireEvent.click(screen.getByTestId("sync-change-folder"));
    fireEvent.click(screen.getByTestId("sync-prompt-browse"));

    await waitFor(() => expect(handleBrowseFolder).toHaveBeenCalled());
    expect(screen.getByDisplayValue("/home/user/sync")).toBeInTheDocument();
  });

  it("opens the start-over flow (its own confirmation lives in the enable modal, SYN-071)", () => {
    const openStartOverModal = vi.fn();
    mockUseSyncSection.mockReturnValue(makeState({ enabled: true, openStartOverModal }));
    render(<SyncSection />);

    fireEvent.click(screen.getByTestId("sync-start-over"));

    expect(openStartOverModal).toHaveBeenCalled();
  });
});
