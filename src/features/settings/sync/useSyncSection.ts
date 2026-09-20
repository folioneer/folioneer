import { useCallback, useEffect, useState } from "react";
import type {
  ConflictNotice,
  InconsistentHolding,
  PortfolioSyncError,
  Result,
  SyncFailure,
  SyncStatus,
} from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";
import {
  changeSyncFolder,
  getSyncStatus,
  leaveSync,
  pauseSync,
  pickSyncFolder,
  renameSyncDevice,
  resumeSync,
  syncNow,
} from "../gateway";
import { type RosterEntryViewModel, rosterToViewModel, syncErrorToI18n } from "../shared/presenter";

export interface UseSyncSectionResult {
  isLoading: boolean;
  loadError: I18nMessage | null;
  enabled: boolean;
  paused: boolean;
  deviceName: string | null;
  /** Version of the application running on this computer (SYN-063). */
  appVersion: string;
  folder: string | null;
  lastSyncCompletedAt: string | null;
  roster: RosterEntryViewModel[];
  heldBackCount: number;
  oldestHeldBackSince: string | null;
  notices: ConflictNotice[];
  inconsistentHoldings: InconsistentHolding[];
  failures: SyncFailure[];
  /** True while a run started from this section is in flight (SYN-061). */
  isSyncing: boolean;
  /** Set when the last action was rejected; cleared by the next action (F27). */
  actionError: I18nMessage | null;
  handleSyncNow: () => Promise<void>;
  handlePause: () => Promise<void>;
  handleResume: () => Promise<void>;
  /** Resolve to true when the backend accepted the change; false leaves `actionError` set. */
  handleRename: (deviceName: string) => Promise<boolean>;
  handleChangeFolder: (folder: string) => Promise<boolean>;
  /** SYN-074 — native folder picker; resolves to the chosen path, or null when cancelled. */
  handleBrowseFolder: () => Promise<string | null>;
  confirmingLeave: boolean;
  requestLeave: () => void;
  cancelLeave: () => void;
  confirmLeave: () => Promise<void>;
  isEnableModalOpen: boolean;
  openEnableModal: () => void;
  closeEnableModal: () => void;
  isStartOverModalOpen: boolean;
  openStartOverModal: () => void;
  closeStartOverModal: () => void;
  /** Re-reads the status after the enable / start-over modal succeeded or a notice was dismissed. */
  refresh: () => Promise<void>;
}

const DISABLED_STATUS: SyncStatus = {
  enabled: false,
  paused: false,
  device_id: null,
  device_name: null,
  folder: null,
  app_version: "",
  last_sync_completed_at: null,
  roster: [],
  held_back_count: 0,
  oldest_held_back_since: null,
  notices: [],
  inconsistent_holdings: [],
  failures: [],
};

/**
 * SYN-061/063/070–074/082 — Settings section state: loads the sync status on
 * mount and exposes every device-side action. Each action's result carries the
 * fresh status (or the run report's status), which replaces the displayed one.
 */
export function useSyncSection(): UseSyncSectionResult {
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<I18nMessage | null>(null);
  const [status, setStatus] = useState<SyncStatus>(DISABLED_STATUS);
  const [isSyncing, setIsSyncing] = useState(false);
  const [actionError, setActionError] = useState<I18nMessage | null>(null);
  const [confirmingLeave, setConfirmingLeave] = useState(false);
  const [isEnableModalOpen, setIsEnableModalOpen] = useState(false);
  const [isStartOverModalOpen, setIsStartOverModalOpen] = useState(false);

  const refresh = useCallback(async () => {
    const result = await getSyncStatus();
    if (result.status === "ok") {
      setStatus(result.data);
      setLoadError(null);
    } else {
      setLoadError(syncErrorToI18n(result.error));
    }
    setIsLoading(false);
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const applyStatusResult = useCallback(
    async (call: () => Promise<Result<SyncStatus, PortfolioSyncError>>): Promise<boolean> => {
      setActionError(null);
      const result = await call();
      if (result.status === "ok") {
        setStatus(result.data);
        return true;
      }
      setActionError(syncErrorToI18n(result.error));
      return false;
    },
    [],
  );

  const handleSyncNow = useCallback(async () => {
    setIsSyncing(true);
    await applyStatusResult(async () => {
      const result = await syncNow();
      return result.status === "ok" ? { status: "ok", data: result.data.status } : result;
    });
    setIsSyncing(false);
  }, [applyStatusResult]);

  const handlePause = useCallback(async () => {
    await applyStatusResult(pauseSync);
  }, [applyStatusResult]);

  const handleResume = useCallback(async () => {
    await applyStatusResult(async () => {
      const result = await resumeSync();
      return result.status === "ok" ? { status: "ok", data: result.data.status } : result;
    });
  }, [applyStatusResult]);

  const handleRename = useCallback(
    (deviceName: string) => applyStatusResult(() => renameSyncDevice(deviceName)),
    [applyStatusResult],
  );

  const handleChangeFolder = useCallback(
    (folder: string) => applyStatusResult(() => changeSyncFolder(folder)),
    [applyStatusResult],
  );

  const handleBrowseFolder = useCallback(() => pickSyncFolder(), []);

  const confirmLeave = useCallback(async () => {
    setActionError(null);
    const result = await leaveSync();
    if (result.status === "ok") {
      setStatus(DISABLED_STATUS);
    } else {
      setActionError(syncErrorToI18n(result.error));
    }
    setConfirmingLeave(false);
  }, []);

  return {
    isLoading,
    loadError,
    enabled: status.enabled,
    paused: status.paused,
    deviceName: status.device_name,
    appVersion: status.app_version,
    folder: status.folder,
    lastSyncCompletedAt: status.last_sync_completed_at,
    roster: rosterToViewModel(status.roster),
    heldBackCount: status.held_back_count,
    oldestHeldBackSince: status.oldest_held_back_since,
    notices: status.notices,
    inconsistentHoldings: status.inconsistent_holdings,
    failures: status.failures,
    isSyncing,
    actionError,
    handleSyncNow,
    handlePause,
    handleResume,
    handleRename,
    handleChangeFolder,
    handleBrowseFolder,
    confirmingLeave,
    requestLeave: () => setConfirmingLeave(true),
    cancelLeave: () => setConfirmingLeave(false),
    confirmLeave,
    isEnableModalOpen,
    openEnableModal: () => setIsEnableModalOpen(true),
    closeEnableModal: () => setIsEnableModalOpen(false),
    isStartOverModalOpen,
    openStartOverModal: () => setIsStartOverModalOpen(true),
    closeStartOverModal: () => setIsStartOverModalOpen(false),
    refresh,
  };
}
