import type {
  ConflictNotice,
  FolderProblem,
  PortfolioSyncError,
  RosterEntry,
  ScheduledFetchError,
  ScheduledFetchRun,
  SyncFailure,
} from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";

/**
 * F27 — Maps a `ScheduledFetchError` to an i18n key. Pure: no React, no
 * `useTranslation`. The exhaustive switch on `code` lets TypeScript catch any
 * new variant at compile time. Keys live under the `error.scheduled_fetch.*`
 * namespace so they don't collide with the shared `error.*` codes used by
 * other bounded contexts.
 */
export function scheduledFetchErrorToI18n(error: ScheduledFetchError): I18nMessage {
  switch (error.code) {
    case "InvalidTriggerTime":
    case "ScheduleRegistrationFailed":
    case "ScheduleRemovalFailed":
    case "DatabaseError":
      return { key: `error.scheduled_fetch.${error.code}` };
    default: {
      const _exhaustive: never = error;
      return _exhaustive;
    }
  }
}

/**
 * SPF-052 — Formats the Settings section's status line from the most recent
 * run: when it ran, its outcome, and its counts. `null` (fresh install, no
 * run has ever executed) maps to a distinct "no download yet" key.
 */
export function formatScheduledFetchStatusLine(lastRun: ScheduledFetchRun | null): I18nMessage {
  if (lastRun === null) {
    return { key: "settings.scheduled_fetch.status_none" };
  }
  switch (lastRun.outcome) {
    case "Succeeded":
      return {
        key: "settings.scheduled_fetch.status_succeeded",
        vars: {
          executedAt: lastRun.executed_at,
          updatedCount: lastRun.updated_count,
          skippedCount: lastRun.skipped_count,
        },
      };
    case "Failed":
      return {
        key: "settings.scheduled_fetch.status_failed",
        vars: { executedAt: lastRun.executed_at },
      };
    case "SkippedAlreadyRun":
      return {
        key: "settings.scheduled_fetch.status_skipped",
        vars: { executedAt: lastRun.executed_at },
      };
    default: {
      const _exhaustive: never = lastRun.outcome;
      return _exhaustive;
    }
  }
}

/**
 * F27 — Maps the error surface of the eleven sync commands (`SyncError` plus
 * the orchestrator's `PortfolioSyncTask` codes) to `sync.errors.*` keys.
 * `UpdateRequired` and `FolderUnavailable` share their keys with the
 * `SyncFailure` twins below — one condition, two wire shapes. A code from
 * another bounded context reaching this surface falls back to the generic key.
 */
export function syncErrorToI18n(error: PortfolioSyncError): I18nMessage {
  switch (error.code) {
    case "AlreadyEnabled":
    case "SyncDisabled":
    case "SyncPaused":
    case "AlreadyPaused":
    case "NotPaused":
    case "DeviceNameBlank":
    case "HeaderRejected":
    case "PassphraseMismatch":
    case "PortfolioCreatedElsewhere":
    case "FolderHoldsOtherPortfolio":
    case "NoticeNotFound":
    case "DatabaseError":
    case "InstallationHoldsUserData":
    case "HistoryIncomplete":
    case "RebuildInterrupted":
    case "UnknownError":
      return { key: `sync.errors.${error.code}` };
    case "PassphraseTooShort":
      return { key: "sync.errors.PassphraseTooShort", vars: { minimum: error.minimum } };
    case "FolderUnavailable":
    case "PublishFailed":
      return { key: `sync.errors.${error.code}`, vars: { problem: error.problem } };
    case "UpdateRequired":
      return {
        key: "sync.errors.UpdateRequired",
        vars: { dataFormatVersion: error.data_format_version },
      };
    default:
      return { key: "error.Unknown" };
  }
}

/** F27 — Maps one `SyncFailure` of a run or status (SYN-034/035/069/084). */
export function syncFailureToI18n(failure: SyncFailure): I18nMessage {
  if (failure === "PortfolioReset") {
    return { key: "sync.failures.PortfolioReset" };
  }
  if ("UnreadableFiles" in failure) {
    return { key: "sync.failures.UnreadableFiles", vars: { count: failure.UnreadableFiles.count } };
  }
  if ("UpdateRequired" in failure) {
    return {
      key: "sync.errors.UpdateRequired",
      vars: { dataFormatVersion: failure.UpdateRequired.data_format_version },
    };
  }
  return {
    key: "sync.errors.FolderUnavailable",
    vars: { problem: failure.FolderUnavailable.problem },
  };
}

/** F27 — Maps a `FolderProblem` reported by `inspect_sync_folder` (SYN-019/069). */
export function folderProblemToI18n(problem: FolderProblem): I18nMessage {
  return { key: `sync.folder_problem.${problem}` };
}

/** F27 — Maps a conflict notice to its sentence (SYN-066, CFR-060). */
export function noticeToI18n(notice: ConflictNotice): I18nMessage {
  return {
    key: `sync.notices.${notice.kind}`,
    vars: { recordLabel: notice.record_label, otherDeviceName: notice.other_device_name },
  };
}

export interface RosterEntryViewModel {
  deviceId: string;
  deviceName: string;
  dataFormatVersion: number;
  /** Version of the application that last published from it; null when it never said (SYN-037). */
  appVersion: string | null;
  /** ISO timestamp of the last time this device's changes were applied here, or null (SYN-063). */
  lastAppliedAt: string | null;
}

/** SYN-037/063 — the other devices of the shared folder. */
export function rosterToViewModel(roster: RosterEntry[]): RosterEntryViewModel[] {
  return roster.map((entry) => ({
    deviceId: entry.device_id,
    deviceName: entry.device_name,
    dataFormatVersion: entry.data_format_version,
    appVersion: entry.app_version,
    lastAppliedAt: entry.last_applied_at,
  }));
}
