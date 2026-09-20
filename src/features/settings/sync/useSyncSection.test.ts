import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SyncReport, SyncStatus } from "@/bindings";

// 1. Mock the gateway module before importing the hook (test_convention.md § Mocking gateway modules)
vi.mock("../gateway", () => ({
  getSyncStatus: vi.fn(),
  syncNow: vi.fn(),
  pauseSync: vi.fn(),
  resumeSync: vi.fn(),
  renameSyncDevice: vi.fn(),
  changeSyncFolder: vi.fn(),
  pickSyncFolder: vi.fn(),
  leaveSync: vi.fn(),
}));

// 2. Import mocked modules for typed access
import * as gateway from "../gateway";
import { useSyncSection } from "./useSyncSection";

function makeSyncStatus(overrides: Partial<SyncStatus> = {}): SyncStatus {
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
      },
    ],
    held_back_count: 0,
    oldest_held_back_since: null,
    notices: [],
    inconsistent_holdings: [],
    failures: [],
    ...overrides,
  };
}

function makeSyncReport(overrides: Partial<SyncReport> = {}): SyncReport {
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

  it("sets isSyncing while the call is in flight", async () => {
    let resolveSync!: (v: { status: "ok"; data: SyncReport }) => void;
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
