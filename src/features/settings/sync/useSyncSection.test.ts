import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SyncReportView, SyncStatusView } from "@/bindings";

// 1. Mock the gateway module before importing the hook (test-rules.md § Mocking gateway modules)
vi.mock("../gateway", () => ({
  getSyncStatus: vi.fn(),
  syncNow: vi.fn(),
  pauseSync: vi.fn(),
  resumeSync: vi.fn(),
  renameSyncDevice: vi.fn(),
  changeSyncFolder: vi.fn(),
  inspectSyncFolder: vi.fn(),
  pickSyncFolder: vi.fn(),
  leaveSync: vi.fn(),
}));

vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

// 2. Import mocked modules for typed access
import * as gateway from "../gateway";
import { useSyncSection } from "./useSyncSection";

function makeSyncStatus(overrides: Partial<SyncStatusView> = {}): SyncStatusView {
  return {
    enabled: true,
    paused: false,
    device_id: "device-1",
    device_name: "Desktop",
    folder: "/home/user/sync",
    app_version: "0.43.0",
    last_sync_completed_at: "2026-08-20T10:00:00Z",
    roster: [
      {
        device_id: "device-2",
        device_name: "Laptop",
        data_format_version: 3,
        app_version: "0.42.0",
        last_applied_at: null,
        published_changes: 3,
      },
    ],
    held_back_count: 0,
    oldest_held_back_since: null,
    notices: [],
    inconsistent_holdings: [],
    failures: [],
    health: "up_to_date",
    ...overrides,
  };
}

function makeSyncReport(overrides: Partial<SyncReportView> = {}): SyncReportView {
  return {
    published_changes: 1,
    applied_changes: 0,
    held_back_changes: 0,
    dropped_changes: 0,
    notices_raised: 0,
    failures: [],
    completed_at: "2026-08-21T09:00:00Z",
    status: makeSyncStatus({ last_sync_completed_at: "2026-08-21T09:00:00Z" }),
    ...overrides,
  };
}

describe("useSyncSection — load status on mount (SYN-063)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is loading before the status call resolves", () => {
    vi.mocked(gateway.getSyncStatus).mockReturnValue(new Promise(() => {}));

    const { result } = renderHook(() => useSyncSection());

    expect(result.current.isLoading).toBe(true);
  });

  // SYN-063 — the page carries the health the core states with the status, as it is.
  it("carries the health the core states", async () => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({
      status: "ok",
      data: makeSyncStatus({ health: "needs_attention" }),
    });

    const { result } = renderHook(() => useSyncSection());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.health).toBe("needs_attention");
  });

  it("loads the disabled status by default (SYN-010)", async () => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({
      status: "ok",
      data: makeSyncStatus({
        enabled: false,
        paused: false,
        device_id: null,
        device_name: null,
        folder: null,
        last_sync_completed_at: null,
      }),
    });

    const { result } = renderHook(() => useSyncSection());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.enabled).toBe(false);
  });

  it("loads the enabled status with device name, folder and roster", async () => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({ status: "ok", data: makeSyncStatus() });

    const { result } = renderHook(() => useSyncSection());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.enabled).toBe(true);
    expect(result.current.deviceName).toBe("Desktop");
    expect(result.current.appVersion).toBe("0.43.0");
    expect(result.current.roster[0]?.appVersion).toBe("0.42.0");
    expect(result.current.lastSyncCompletedAt).toBe("2026-08-20T10:00:00Z");
    expect(result.current.roster).toHaveLength(1);
  });

  it("sets loadError to the presented error message on load failure", async () => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    const { result } = renderHook(() => useSyncSection());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.loadError).toEqual({ key: "sync.errors.DatabaseError" });
  });
});

describe("useSyncSection — Sync now (SYN-061)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({ status: "ok", data: makeSyncStatus() });
  });

  it("calls syncNow and re-renders status from report.status on success", async () => {
    vi.mocked(gateway.syncNow).mockResolvedValue({ status: "ok", data: makeSyncReport() });

    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.handleSyncNow();
    });

    expect(gateway.syncNow).toHaveBeenCalledWith();
    expect(result.current.lastSyncCompletedAt).toBe("2026-08-21T09:00:00Z");
  });

  // SYN-063 — the health follows the status a run reports, as the core states it.
  it("takes the health of the status a run reports", async () => {
    vi.mocked(gateway.syncNow).mockResolvedValue({
      status: "ok",
      data: makeSyncReport({ status: makeSyncStatus({ health: "needs_attention" }) }),
    });

    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.health).toBe("up_to_date");

    await act(async () => {
      await result.current.handleSyncNow();
    });

    expect(result.current.health).toBe("needs_attention");
  });

  it("sets isSyncing while the call is in flight", async () => {
    let resolveSync!: (v: { status: "ok"; data: SyncReportView }) => void;
    vi.mocked(gateway.syncNow).mockReturnValue(
      new Promise((resolve) => {
        resolveSync = resolve;
      }),
    );

    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => {
      void result.current.handleSyncNow();
    });
    expect(result.current.isSyncing).toBe(true);

    await act(async () => resolveSync({ status: "ok", data: makeSyncReport() }));
    expect(result.current.isSyncing).toBe(false);
  });

  it("renders the presented error inline when syncNow is rejected (SYN-070)", async () => {
    vi.mocked(gateway.syncNow).mockResolvedValue({
      status: "error",
      error: { code: "SyncPaused" },
    });

    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.handleSyncNow();
    });

    expect(result.current.actionError).toEqual({ key: "sync.errors.SyncPaused" });
  });
});

describe("useSyncSection — pause / resume (SYN-070/073)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({ status: "ok", data: makeSyncStatus() });
  });

  it("calls pauseSync and updates paused state", async () => {
    vi.mocked(gateway.pauseSync).mockResolvedValue({
      status: "ok",
      data: makeSyncStatus({ paused: true }),
    });

    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.handlePause();
    });

    expect(gateway.pauseSync).toHaveBeenCalledWith();
    expect(result.current.paused).toBe(true);
  });

  it("calls resumeSync and updates paused state from the report", async () => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({
      status: "ok",
      data: makeSyncStatus({ paused: true }),
    });
    vi.mocked(gateway.resumeSync).mockResolvedValue({
      status: "ok",
      data: makeSyncReport({ status: makeSyncStatus({ paused: false }) }),
    });

    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.paused).toBe(true);

    await act(async () => {
      await result.current.handleResume();
    });

    expect(gateway.resumeSync).toHaveBeenCalledWith();
    expect(result.current.paused).toBe(false);
  });
});

describe("useSyncSection — rename device / change folder (SYN-072/074)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({ status: "ok", data: makeSyncStatus() });
  });

  it("calls renameSyncDevice with the new name", async () => {
    vi.mocked(gateway.renameSyncDevice).mockResolvedValue({
      status: "ok",
      data: makeSyncStatus({ device_name: "Laptop" }),
    });

    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.handleRename("Laptop");
    });

    expect(gateway.renameSyncDevice).toHaveBeenCalledWith("Laptop");
    expect(result.current.deviceName).toBe("Laptop");
  });

  it("calls changeSyncFolder with the new folder", async () => {
    vi.mocked(gateway.changeSyncFolder).mockResolvedValue({
      status: "ok",
      data: makeSyncStatus({ folder: "/home/user/new-sync" }),
    });

    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.handleChangeFolder("/home/user/new-sync");
    });

    expect(gateway.changeSyncFolder).toHaveBeenCalledWith("/home/user/new-sync");
    expect(result.current.folder).toBe("/home/user/new-sync");
  });

  // SYN-074 — a folder the portfolio was moved to by another device is refused as a join
  // on an installation that holds data: the reason is said and the folder stays.
  it("says why a moved portfolio's folder is refused, and keeps the folder", async () => {
    vi.mocked(gateway.changeSyncFolder).mockResolvedValue({
      status: "error",
      error: { code: "InstallationHoldsUserData" },
    } as never);

    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    const before = result.current.folder;

    await act(async () => {
      await result.current.handleChangeFolder("/home/user/moved-elsewhere");
    });

    expect(result.current.actionError).toEqual({ key: "sync.errors.InstallationHoldsUserData" });
    expect(result.current.folder).toBe(before);
  });
});

describe("useSyncSection — moving to a folder that holds no portfolio (SYN-074)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({ status: "ok", data: makeSyncStatus() });
    vi.mocked(gateway.changeSyncFolder).mockResolvedValue({
      status: "ok",
      data: makeSyncStatus({ folder: "/media/key/Folioneer" }),
    });
  });

  // SYN-075 — a folder that holds no portfolio asks to confirm before moving.
  it("asks to confirm before moving to a folder that holds no portfolio", async () => {
    vi.mocked(gateway.inspectSyncFolder).mockResolvedValue({
      status: "ok",
      data: { problem: null, holds_portfolio: false },
    } as never);
    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    let accepted = true;
    await act(async () => {
      accepted = await result.current.handleChangeFolder("/media/key/Folioneer");
    });
    expect(accepted).toBe(false);
    expect(gateway.changeSyncFolder).not.toHaveBeenCalled();
    expect(result.current.pendingMove).toBe("/media/key/Folioneer");

    let moved = false;
    await act(async () => {
      moved = await result.current.confirmMove();
    });
    expect(moved).toBe(true);
    expect(gateway.changeSyncFolder).toHaveBeenCalledWith("/media/key/Folioneer");
    expect(result.current.pendingMove).toBeNull();
    expect(result.current.folder).toBe("/media/key/Folioneer");
  });

  // TODO-082 — keeping the current folder moves nothing.
  it("keeps the current folder when the move is not confirmed", async () => {
    vi.mocked(gateway.inspectSyncFolder).mockResolvedValue({
      status: "ok",
      data: { problem: null, holds_portfolio: false },
    } as never);
    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.handleChangeFolder("/media/key/Folioneer");
    });
    act(() => result.current.cancelMove());
    expect(result.current.pendingMove).toBeNull();
    expect(gateway.changeSyncFolder).not.toHaveBeenCalled();
  });

  // SYN-075 — a folder that could not be read goes to the change, which says why.
  it("changes to a folder that could not be read without asking", async () => {
    vi.mocked(gateway.inspectSyncFolder).mockRejectedValue(new Error("no core"));
    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.handleChangeFolder("/media/key/Folioneer");
    });
    expect(result.current.pendingMove).toBeNull();
    expect(gateway.changeSyncFolder).toHaveBeenCalledWith("/media/key/Folioneer");
  });

  // SYN-074 — a folder that holds the portfolio followed is changed to with no question.
  it("changes to a folder that holds a portfolio without asking", async () => {
    vi.mocked(gateway.inspectSyncFolder).mockResolvedValue({
      status: "ok",
      data: { problem: null, holds_portfolio: true },
    } as never);
    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.handleChangeFolder("/media/key/Folioneer");
    });
    expect(result.current.pendingMove).toBeNull();
    expect(gateway.changeSyncFolder).toHaveBeenCalledWith("/media/key/Folioneer");
  });
});

describe("useSyncSection — leave sync confirmation (SYN-071/082)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({ status: "ok", data: makeSyncStatus() });
  });

  it("does not call leaveSync until confirmed", async () => {
    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.requestLeave());

    expect(result.current.confirmingLeave).toBe(true);
    expect(gateway.leaveSync).not.toHaveBeenCalled();
  });

  it("calls leaveSync only after confirmLeave", async () => {
    vi.mocked(gateway.leaveSync).mockResolvedValue({ status: "ok", data: null });

    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.requestLeave());
    await act(async () => {
      await result.current.confirmLeave();
    });

    expect(gateway.leaveSync).toHaveBeenCalledWith();
    expect(result.current.confirmingLeave).toBe(false);
  });

  it("cancelLeave dismisses the confirmation without calling leaveSync", async () => {
    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.requestLeave());
    act(() => result.current.cancelLeave());

    expect(result.current.confirmingLeave).toBe(false);
    expect(gateway.leaveSync).not.toHaveBeenCalled();
  });
  it("handleBrowseFolder returns the picked path (SYN-074)", async () => {
    vi.mocked(gateway.pickSyncFolder).mockResolvedValue("/media/phil/KEY/Folioneer");

    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    let picked: string | null = null;
    await act(async () => {
      picked = await result.current.handleBrowseFolder();
    });

    expect(picked).toBe("/media/phil/KEY/Folioneer");
    expect(gateway.pickSyncFolder).toHaveBeenCalledWith();
  });

  it("handleBrowseFolder passes a cancelled picker through as null (SYN-074)", async () => {
    vi.mocked(gateway.pickSyncFolder).mockResolvedValue(null);

    const { result } = renderHook(() => useSyncSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    let picked: string | null = "unchanged";
    await act(async () => {
      picked = await result.current.handleBrowseFolder();
    });

    expect(picked).toBeNull();
  });
});
