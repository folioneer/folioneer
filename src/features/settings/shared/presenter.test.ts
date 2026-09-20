import { describe, expect, it } from "vitest";
import type {
  ConflictNotice,
  FolderProblem,
  PortfolioSyncError,
  RosterEntry,
  ScheduledFetchError,
  ScheduledFetchRun,
  SyncFailure,
} from "@/bindings";
// SYN — namespace import (not named) so a not-yet-implemented sync presenter
// function fails at its individual call site (a graceful per-test TypeError)
// instead of crashing the whole module load and taking the scheduled-fetch
// tests above down with it (mirrors the gateway.test.ts dynamic-import idiom).
import * as presenter from "./presenter";
import { formatScheduledFetchStatusLine, scheduledFetchErrorToI18n } from "./presenter";

// ---------------------------------------------------------------------------
// scheduledFetchErrorToI18n — F27 error → i18n key (one test per ScheduledFetchError variant)
// ---------------------------------------------------------------------------

describe("scheduledFetchErrorToI18n", () => {
  // SPF-019 — malformed trigger time
  it("maps InvalidTriggerTime to its flat i18n key (SPF-019)", () => {
    const error: ScheduledFetchError = { code: "InvalidTriggerTime" };
    expect(scheduledFetchErrorToI18n(error)).toEqual({
      key: "error.scheduled_fetch.InvalidTriggerTime",
    });
  });

  // SPF-013 — OS schedule registration failure
  it("maps ScheduleRegistrationFailed to its flat i18n key (SPF-013)", () => {
    const error: ScheduledFetchError = { code: "ScheduleRegistrationFailed" };
    expect(scheduledFetchErrorToI18n(error)).toEqual({
      key: "error.scheduled_fetch.ScheduleRegistrationFailed",
    });
  });

  // SPF-013 — OS schedule removal failure
  it("maps ScheduleRemovalFailed to its flat i18n key (SPF-013)", () => {
    const error: ScheduledFetchError = { code: "ScheduleRemovalFailed" };
    expect(scheduledFetchErrorToI18n(error)).toEqual({
      key: "error.scheduled_fetch.ScheduleRemovalFailed",
    });
  });

  // infrastructure failure
  it("maps DatabaseError to its flat i18n key", () => {
    const error: ScheduledFetchError = { code: "DatabaseError" };
    expect(scheduledFetchErrorToI18n(error)).toEqual({
      key: "error.scheduled_fetch.DatabaseError",
    });
  });
});

// ---------------------------------------------------------------------------
// formatScheduledFetchStatusLine — SPF-052: "when it ran, outcome, counts"
// ---------------------------------------------------------------------------

describe("formatScheduledFetchStatusLine (SPF-052)", () => {
  // Fresh install — no run has ever executed
  it("returns the no-download-yet key when lastRun is null", () => {
    expect(formatScheduledFetchStatusLine(null)).toEqual({
      key: "settings.scheduled_fetch.status_none",
    });
  });

  // Succeeded outcome carries executedAt + both counts
  it("returns the succeeded key with executedAt/updatedCount/skippedCount for a successful run", () => {
    const run: ScheduledFetchRun = {
      executed_at: "2026-07-12T19:00:00Z",
      trigger_date: "2026-07-12",
      outcome: "Succeeded",
      updated_count: 12,
      skipped_count: 2,
    };
    expect(formatScheduledFetchStatusLine(run)).toEqual({
      key: "settings.scheduled_fetch.status_succeeded",
      vars: { executedAt: "2026-07-12T19:00:00Z", updatedCount: 12, skippedCount: 2 },
    });
  });

  // Failed outcome — shown inline here only, no popup (SPF-052)
  it("returns the failed key with executedAt for a failed run", () => {
    const run: ScheduledFetchRun = {
      executed_at: "2026-07-12T19:00:00Z",
      trigger_date: "2026-07-12",
      outcome: "Failed",
      updated_count: 0,
      skipped_count: 0,
    };
    expect(formatScheduledFetchStatusLine(run)).toEqual({
      key: "settings.scheduled_fetch.status_failed",
      vars: { executedAt: "2026-07-12T19:00:00Z" },
    });
  });

  // SkippedAlreadyRun outcome — once-per-day guard exit
  it("returns the skipped key with executedAt for a guard-skipped run", () => {
    const run: ScheduledFetchRun = {
      executed_at: "2026-07-12T19:00:00Z",
      trigger_date: "2026-07-12",
      outcome: "SkippedAlreadyRun",
      updated_count: 0,
      skipped_count: 0,
    };
    expect(formatScheduledFetchStatusLine(run)).toEqual({
      key: "settings.scheduled_fetch.status_skipped",
      vars: { executedAt: "2026-07-12T19:00:00Z" },
    });
  });
});

// ---------------------------------------------------------------------------
// syncErrorToI18n — F27 presenter for every command's PortfolioSyncError/
// SyncError surface (SYN-011/012/014/015/018/019/035/051/053/066/071/074/081,
// PortfolioSyncTask). One test per reachable code; `UpdateRequired` and
// `FolderUnavailable` map to the SAME key as their `SyncFailure` twin
// (contract note: "one condition, two wire shapes").
// ---------------------------------------------------------------------------

describe("syncErrorToI18n", () => {
  it.each([
    "AlreadyEnabled",
    "SyncDisabled",
    "SyncPaused",
    "AlreadyPaused",
    "NotPaused",
    "DeviceNameBlank",
    "HeaderRejected",
    "PassphraseMismatch",
    "PortfolioCreatedElsewhere",
    "FolderHoldsOtherPortfolio",
    "DatabaseError",
    // PortfolioSyncTask codes — orchestrator-level guards + catch-all
    "InstallationHoldsUserData",
    "HistoryIncomplete",
    "RebuildInterrupted",
    "UnknownError",
  ] as const)("maps %s to its flat sync error key", (code) => {
    expect(presenter.syncErrorToI18n({ code } as PortfolioSyncError)).toEqual({
      key: `sync.errors.${code}`,
    });
  });

  // SYN-012 — carries the minimum length for interpolation
  it("maps PassphraseTooShort with the minimum length payload", () => {
    expect(
      presenter.syncErrorToI18n({ code: "PassphraseTooShort", minimum: 12 } as PortfolioSyncError),
    ).toEqual({ key: "sync.errors.PassphraseTooShort", vars: { minimum: 12 } });
  });

  // SYN-066 — the id is not user-facing; the message alone suffices
  it("maps NoticeNotFound to its flat key without the notice_id payload", () => {
    expect(
      presenter.syncErrorToI18n({
        code: "NoticeNotFound",
        notice_id: "notice-1",
      } as PortfolioSyncError),
    ).toEqual({ key: "sync.errors.NoticeNotFound" });
  });

  // SYN-019/069 — carries the structured FolderProblem
  it("maps FolderUnavailable with the problem payload", () => {
    expect(
      presenter.syncErrorToI18n({
        code: "FolderUnavailable",
        problem: "PermissionDenied",
      } as PortfolioSyncError),
    ).toEqual({ key: "sync.errors.FolderUnavailable", vars: { problem: "PermissionDenied" } });
  });

  // SYN-013 — carries the structured FolderProblem
  it("maps PublishFailed with the problem payload", () => {
    expect(
      presenter.syncErrorToI18n({
        code: "PublishFailed",
        problem: "OutOfSpace",
      } as PortfolioSyncError),
    ).toEqual({ key: "sync.errors.PublishFailed", vars: { problem: "OutOfSpace" } });
  });

  // SYN-019/035 — one condition, two wire shapes: same key as SyncFailure::UpdateRequired
  it("maps the UpdateRequired error code to the shared sync.errors.UpdateRequired key", () => {
    expect(
      presenter.syncErrorToI18n({
        code: "UpdateRequired",
        data_format_version: 3,
      } as PortfolioSyncError),
    ).toEqual({ key: "sync.errors.UpdateRequired", vars: { dataFormatVersion: 3 } });
  });

  // An AccountError/AssetError/CurrencyError code reaching this command's error
  // surface is out of this presenter's scope — falls back to a generic key.
  it("falls back to a generic key for a non-sync code", () => {
    expect(
      presenter.syncErrorToI18n({ code: "AccountNotFound", account_id: "a" } as never),
    ).toEqual({ key: "error.Unknown" });
  });
});

// ---------------------------------------------------------------------------
// syncFailureToI18n — F27 presenter for SyncStatus/SyncReport.failures
// (SYN-034/035/069/084)
// ---------------------------------------------------------------------------

describe("syncFailureToI18n", () => {
  it("maps UnreadableFiles with the skipped count", () => {
    const failure: SyncFailure = { UnreadableFiles: { count: 3 } };
    expect(presenter.syncFailureToI18n(failure)).toEqual({
      key: "sync.failures.UnreadableFiles",
      vars: { count: 3 },
    });
  });

  // Same key as the UpdateRequired error code (contract: "one condition, two wire shapes")
  it("maps UpdateRequired to the shared sync.errors.UpdateRequired key", () => {
    const failure: SyncFailure = { UpdateRequired: { data_format_version: 4 } };
    expect(presenter.syncFailureToI18n(failure)).toEqual({
      key: "sync.errors.UpdateRequired",
      vars: { dataFormatVersion: 4 },
    });
  });

  // Same key as the FolderUnavailable error code
  it("maps FolderUnavailable to the shared sync.errors.FolderUnavailable key", () => {
    const failure: SyncFailure = { FolderUnavailable: { problem: "Unmounted" } };
    expect(presenter.syncFailureToI18n(failure)).toEqual({
      key: "sync.errors.FolderUnavailable",
      vars: { problem: "Unmounted" },
    });
  });

  it("maps the bare PortfolioReset marker (SYN-084)", () => {
    const failure: SyncFailure = "PortfolioReset";
    expect(presenter.syncFailureToI18n(failure)).toEqual({ key: "sync.failures.PortfolioReset" });
  });
});

// ---------------------------------------------------------------------------
// folderProblemToI18n — F27 presenter for FolderProblem (SYN-019/069)
// ---------------------------------------------------------------------------

describe("folderProblemToI18n", () => {
  it.each([
    "Missing",
    "NotADirectory",
    "PermissionDenied",
    "Unmounted",
    "OutOfSpace",
    "IoFailure",
  ] as const satisfies readonly FolderProblem[])("maps %s to its flat folder-problem key", (problem) => {
    expect(presenter.folderProblemToI18n(problem)).toEqual({
      key: `sync.folder_problem.${problem}`,
    });
  });
});

// ---------------------------------------------------------------------------
// noticeToI18n — F27 presenter for ConflictNotice (SYN-066, CFR-060). Every
// notice interpolates the record label + the other device's name.
// ---------------------------------------------------------------------------

describe("noticeToI18n", () => {
  const makeNotice = (overrides: Partial<ConflictNotice> = {}): ConflictNotice => ({
    notice_id: "notice-1",
    kind: "OverruledEdit",
    record_kind: "Transaction",
    record_identity: "tx-1",
    record_label: "Sell 10 AAPL on 2026-08-01",
    other_device_id: "device-2",
    other_device_name: "Laptop",
    raised_at: "2026-08-20T10:00:00Z",
    ...overrides,
  });

  it.each([
    "OverruledEdit",
    "OverruledRemoval",
    "DroppedChild",
    "NaturalKeyCollision",
    "DuplicateName",
  ] as const)("maps %s with the record label and other device name interpolated", (kind) => {
    expect(presenter.noticeToI18n(makeNotice({ kind }))).toEqual({
      key: `sync.notices.${kind}`,
      vars: { recordLabel: "Sell 10 AAPL on 2026-08-01", otherDeviceName: "Laptop" },
    });
  });
});

// ---------------------------------------------------------------------------
// rosterToViewModel — SYN-037/063: every other device's manifest state
// ---------------------------------------------------------------------------

describe("rosterToViewModel", () => {
  it("maps each roster entry to a display-ready view model", () => {
    const roster: RosterEntry[] = [
      {
        device_id: "device-2",
        device_name: "Laptop",
        data_format_version: 3,
        app_version: "0.43.0",
        last_applied_at: null,
      },
      {
        device_id: "device-3",
        device_name: "Office",
        data_format_version: 3,
        app_version: null,
        last_applied_at: "2026-08-19T08:00:00Z",
      },
    ];

    expect(presenter.rosterToViewModel(roster)).toEqual([
      {
        deviceId: "device-2",
        deviceName: "Laptop",
        dataFormatVersion: 3,
        appVersion: "0.43.0",
        lastAppliedAt: null,
      },
      {
        deviceId: "device-3",
        deviceName: "Office",
        dataFormatVersion: 3,
        appVersion: null,
        lastAppliedAt: "2026-08-19T08:00:00Z",
      },
    ]);
  });
});
