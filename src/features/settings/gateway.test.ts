import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ScheduledFetchRun,
  ScheduledFetchStatus,
  SyncFolderState,
  SyncReport,
  SyncStatus,
} from "@/bindings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockInvoke = vi.mocked(invoke);

// Import after mock is registered so bindings.ts picks up the mock
const { settingsGateway } = await import("./gateway");

function makeSyncStatus(overrides: Partial<SyncStatus> = {}): SyncStatus {
  return {
    enabled: true,
    paused: false,
    device_id: "device-1",
    device_name: "Desktop",
    folder: "/home/user/sync",
    app_version: "0.43.0",
    last_sync_completed_at: "2026-08-20T10:00:00Z",
    roster: [],
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
    completed_at: "2026-08-20T10:00:00Z",
    status: makeSyncStatus(),
    ...overrides,
  };
}

function makeSyncFolderState(overrides: Partial<SyncFolderState> = {}): SyncFolderState {
  return {
    problem: null,
    holds_portfolio: false,
    data_format_version: null,
    format_readable: true,
    installation_holds_user_data: false,
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// configureScheduledFetch
// ---------------------------------------------------------------------------

describe("settingsGateway — configureScheduledFetch", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // SPF-012 — ok pass-through
  it("configureScheduledFetch passes through ok result (SPF-012)", async () => {
    mockInvoke.mockResolvedValue(null);

    const result = await settingsGateway.configureScheduledFetch(true, "19:00");

    expect(result).toEqual({ status: "ok", data: null });
    expect(mockInvoke).toHaveBeenCalledWith("configure_scheduled_fetch", {
      enabled: true,
      triggerTime: "19:00",
    });
  });

  // SPF-019 — malformed trigger time rejected
  it("configureScheduledFetch passes through InvalidTriggerTime error (SPF-019)", async () => {
    mockInvoke.mockRejectedValue({ code: "InvalidTriggerTime" });

    const result = await settingsGateway.configureScheduledFetch(true, "25:99");

    expect(result).toEqual({ status: "error", error: { code: "InvalidTriggerTime" } });
  });

  // SPF-013 — OS schedule registration failure
  it("configureScheduledFetch passes through ScheduleRegistrationFailed error (SPF-013)", async () => {
    mockInvoke.mockRejectedValue({ code: "ScheduleRegistrationFailed" });

    const result = await settingsGateway.configureScheduledFetch(true, "19:00");

    expect(result).toEqual({ status: "error", error: { code: "ScheduleRegistrationFailed" } });
  });

  // SPF-013 — OS schedule removal failure (disabling)
  it("configureScheduledFetch passes through ScheduleRemovalFailed error (SPF-013)", async () => {
    mockInvoke.mockRejectedValue({ code: "ScheduleRemovalFailed" });

    const result = await settingsGateway.configureScheduledFetch(false, "19:00");

    expect(result).toEqual({ status: "error", error: { code: "ScheduleRemovalFailed" } });
  });

  // infrastructure failure
  it("configureScheduledFetch passes through DatabaseError", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await settingsGateway.configureScheduledFetch(true, "19:00");

    expect(result).toEqual({ status: "error", error: { code: "DatabaseError" } });
  });
});

// ---------------------------------------------------------------------------
// getScheduledFetchStatus
// ---------------------------------------------------------------------------

describe("settingsGateway — getScheduledFetchStatus", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // SPF-052 — ok pass-through, fresh install (no run yet)
  it("getScheduledFetchStatus passes through ok result with no last run (SPF-052)", async () => {
    const status: ScheduledFetchStatus = {
      configuration: { enabled: false, trigger_time: "22:15" },
      last_run: null,
    };
    mockInvoke.mockResolvedValue(status);

    const result = await settingsGateway.getScheduledFetchStatus();

    expect(result).toEqual({ status: "ok", data: status });
    expect(mockInvoke).toHaveBeenCalledWith("get_scheduled_fetch_status");
  });

  // SPF-052 — ok pass-through with a completed run
  it("getScheduledFetchStatus passes through ok result with a completed run", async () => {
    const run: ScheduledFetchRun = {
      executed_at: "2026-07-12T19:00:00Z",
      trigger_date: "2026-07-12",
      outcome: "Succeeded",
      updated_count: 12,
      skipped_count: 2,
    };
    const status: ScheduledFetchStatus = {
      configuration: { enabled: true, trigger_time: "19:00" },
      last_run: run,
    };
    mockInvoke.mockResolvedValue(status);

    const result = await settingsGateway.getScheduledFetchStatus();

    expect(result).toEqual({ status: "ok", data: status });
  });

  // infrastructure failure
  it("getScheduledFetchStatus passes through DatabaseError", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await settingsGateway.getScheduledFetchStatus();

    expect(result).toEqual({ status: "error", error: { code: "DatabaseError" } });
  });
});

// ---------------------------------------------------------------------------
// Sync gateway pass-throughs (SYN — sync-contract.md) — F3/F27: typed
// `Result` pass-through, positional args exactly as bindings.ts declares them.
// ---------------------------------------------------------------------------

describe("settingsGateway — inspectSyncFolder (SYN-011/014/019)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("inspectSyncFolder passes through ok result", async () => {
    const state = makeSyncFolderState({ holds_portfolio: true });
    mockInvoke.mockResolvedValue(state);

    const result = await settingsGateway.inspectSyncFolder("/home/user/sync");

    expect(result).toEqual({ status: "ok", data: state });
    expect(mockInvoke).toHaveBeenCalledWith("inspect_sync_folder", { folder: "/home/user/sync" });
  });

  it("inspectSyncFolder passes through DatabaseError", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await settingsGateway.inspectSyncFolder("/home/user/sync");

    expect(result).toEqual({ status: "error", error: { code: "DatabaseError" } });
  });
});

describe("settingsGateway — enableSync (SYN-011)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("enableSync passes through ok result with positional args", async () => {
    const status = makeSyncStatus();
    mockInvoke.mockResolvedValue(status);

    const result = await settingsGateway.enableSync(
      "/home/user/sync",
      "correct horse battery",
      "Desktop",
    );

    expect(result).toEqual({ status: "ok", data: status });
    expect(mockInvoke).toHaveBeenCalledWith("enable_sync", {
      folder: "/home/user/sync",
      passphrase: "correct horse battery",
      deviceName: "Desktop",
    });
  });

  it("enableSync passes through PassphraseTooShort error", async () => {
    mockInvoke.mockRejectedValue({ code: "PassphraseTooShort", minimum: 12 });

    const result = await settingsGateway.enableSync("/home/user/sync", "short", "Desktop");

    expect(result).toEqual({
      status: "error",
      error: { code: "PassphraseTooShort", minimum: 12 },
    });
  });
});

describe("settingsGateway — startSyncOver (SYN-071)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("startSyncOver passes through ok result with positional args", async () => {
    const status = makeSyncStatus();
    mockInvoke.mockResolvedValue(status);

    const result = await settingsGateway.startSyncOver(
      "/home/user/sync",
      "correct horse battery",
      "Desktop",
    );

    expect(result).toEqual({ status: "ok", data: status });
    expect(mockInvoke).toHaveBeenCalledWith("start_sync_over", {
      folder: "/home/user/sync",
      passphrase: "correct horse battery",
      deviceName: "Desktop",
    });
  });

  it("startSyncOver passes through PublishFailed error", async () => {
    mockInvoke.mockRejectedValue({ code: "PublishFailed", problem: "IoFailure" });

    const result = await settingsGateway.startSyncOver(
      "/home/user/sync",
      "correct horse battery",
      "Desktop",
    );

    expect(result).toEqual({
      status: "error",
      error: { code: "PublishFailed", problem: "IoFailure" },
    });
  });
});

describe("settingsGateway — leaveSync (SYN-082)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("leaveSync passes through ok result with no args", async () => {
    mockInvoke.mockResolvedValue(null);

    const result = await settingsGateway.leaveSync();

    expect(result).toEqual({ status: "ok", data: null });
    expect(mockInvoke).toHaveBeenCalledWith("leave_sync");
  });

  it("leaveSync passes through SyncDisabled error", async () => {
    mockInvoke.mockRejectedValue({ code: "SyncDisabled" });

    const result = await settingsGateway.leaveSync();

    expect(result).toEqual({ status: "error", error: { code: "SyncDisabled" } });
  });
});

describe("settingsGateway — syncNow (SYN-061)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("syncNow passes through ok SyncReport with no args", async () => {
    const report = makeSyncReport();
    mockInvoke.mockResolvedValue(report);

    const result = await settingsGateway.syncNow();

    expect(result).toEqual({ status: "ok", data: report });
    expect(mockInvoke).toHaveBeenCalledWith("sync_now");
  });

  it("syncNow passes through SyncPaused error (SYN-070)", async () => {
    mockInvoke.mockRejectedValue({ code: "SyncPaused" });

    const result = await settingsGateway.syncNow();

    expect(result).toEqual({ status: "error", error: { code: "SyncPaused" } });
  });
});

describe("settingsGateway — pauseSync (SYN-070)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("pauseSync passes through ok SyncStatus with no args", async () => {
    const status = makeSyncStatus({ paused: true });
    mockInvoke.mockResolvedValue(status);

    const result = await settingsGateway.pauseSync();

    expect(result).toEqual({ status: "ok", data: status });
    expect(mockInvoke).toHaveBeenCalledWith("pause_sync");
  });

  it("pauseSync passes through AlreadyPaused error", async () => {
    mockInvoke.mockRejectedValue({ code: "AlreadyPaused" });

    const result = await settingsGateway.pauseSync();

    expect(result).toEqual({ status: "error", error: { code: "AlreadyPaused" } });
  });
});

describe("settingsGateway — resumeSync (SYN-073)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("resumeSync passes through ok SyncReport with no args", async () => {
    const report = makeSyncReport();
    mockInvoke.mockResolvedValue(report);

    const result = await settingsGateway.resumeSync();

    expect(result).toEqual({ status: "ok", data: report });
    expect(mockInvoke).toHaveBeenCalledWith("resume_sync");
  });

  it("resumeSync passes through NotPaused error", async () => {
    mockInvoke.mockRejectedValue({ code: "NotPaused" });

    const result = await settingsGateway.resumeSync();

    expect(result).toEqual({ status: "error", error: { code: "NotPaused" } });
  });
});

describe("settingsGateway — getSyncStatus (SYN-063)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("getSyncStatus passes through ok result with no args", async () => {
    const status = makeSyncStatus();
    mockInvoke.mockResolvedValue(status);

    const result = await settingsGateway.getSyncStatus();

    expect(result).toEqual({ status: "ok", data: status });
    expect(mockInvoke).toHaveBeenCalledWith("get_sync_status");
  });

  it("getSyncStatus passes through DatabaseError", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await settingsGateway.getSyncStatus();

    expect(result).toEqual({ status: "error", error: { code: "DatabaseError" } });
  });
});

describe("settingsGateway — renameSyncDevice (SYN-072)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renameSyncDevice passes through ok result with positional arg", async () => {
    const status = makeSyncStatus({ device_name: "Laptop" });
    mockInvoke.mockResolvedValue(status);

    const result = await settingsGateway.renameSyncDevice("Laptop");

    expect(result).toEqual({ status: "ok", data: status });
    expect(mockInvoke).toHaveBeenCalledWith("rename_sync_device", { deviceName: "Laptop" });
  });

  it("renameSyncDevice passes through DeviceNameBlank error", async () => {
    mockInvoke.mockRejectedValue({ code: "DeviceNameBlank" });

    const result = await settingsGateway.renameSyncDevice("   ");

    expect(result).toEqual({ status: "error", error: { code: "DeviceNameBlank" } });
  });
});

describe("settingsGateway — changeSyncFolder (SYN-074)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("changeSyncFolder passes through ok result with positional arg", async () => {
    const status = makeSyncStatus({ folder: "/home/user/new-sync" });
    mockInvoke.mockResolvedValue(status);

    const result = await settingsGateway.changeSyncFolder("/home/user/new-sync");

    expect(result).toEqual({ status: "ok", data: status });
    expect(mockInvoke).toHaveBeenCalledWith("change_sync_folder", {
      folder: "/home/user/new-sync",
    });
  });

  it("changeSyncFolder passes through FolderHoldsOtherPortfolio error", async () => {
    mockInvoke.mockRejectedValue({ code: "FolderHoldsOtherPortfolio" });

    const result = await settingsGateway.changeSyncFolder("/home/user/other-sync");

    expect(result).toEqual({ status: "error", error: { code: "FolderHoldsOtherPortfolio" } });
  });
});

describe("settingsGateway — dismissConflictNotice (SYN-066)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("dismissConflictNotice passes through ok result with positional arg", async () => {
    mockInvoke.mockResolvedValue(null);

    const result = await settingsGateway.dismissConflictNotice("notice-1");

    expect(result).toEqual({ status: "ok", data: null });
    expect(mockInvoke).toHaveBeenCalledWith("dismiss_conflict_notice", { noticeId: "notice-1" });
  });

  it("dismissConflictNotice passes through NoticeNotFound error", async () => {
    mockInvoke.mockRejectedValue({ code: "NoticeNotFound", notice_id: "notice-1" });

    const result = await settingsGateway.dismissConflictNotice("notice-1");

    expect(result).toEqual({
      status: "error",
      error: { code: "NoticeNotFound", notice_id: "notice-1" },
    });
  });
});
