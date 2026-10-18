import { open } from "@tauri-apps/plugin-dialog";
import {
  type AgentConnectionError,
  type AgentConnectionState,
  type CurrencyError,
  commands,
  type PortfolioSyncError,
  type RateHistoryBackfillError,
  type ReferenceCurrency,
  type Result,
  type ScheduledFetchError,
  type ScheduledFetchStatus,
  type SyncError,
  type SyncFolderState,
  type SyncReportView,
  type SyncStatusView,
} from "../../bindings";

/**
 * Gateway for the Settings feature's scheduled-fetch (SPF-010–061) and sync
 * (SYN) commands. Centralizes all Tauri command calls for the feature (the only
 * file allowed to touch `commands.*`, F3). Each function is a typed `Result`
 * pass-through (F27) matching the `bindings.ts` signature.
 */
export async function configureScheduledFetch(
  enabled: boolean,
  triggerTime: string,
): Promise<Result<null, ScheduledFetchError>> {
  return await commands.configureScheduledFetch(enabled, triggerTime);
}

export async function getScheduledFetchStatus(): Promise<
  Result<ScheduledFetchStatus, ScheduledFetchError>
> {
  return await commands.getScheduledFetchStatus();
}

export async function inspectSyncFolder(
  folder: string,
): Promise<Result<SyncFolderState, PortfolioSyncError>> {
  return await commands.inspectSyncFolder(folder);
}

export async function enableSync(
  folder: string,
  passphrase: string,
  deviceName: string,
): Promise<Result<SyncStatusView, PortfolioSyncError>> {
  return await commands.enableSync(folder, passphrase, deviceName);
}

export async function startSyncOver(
  folder: string,
  passphrase: string,
  deviceName: string,
): Promise<Result<SyncStatusView, PortfolioSyncError>> {
  return await commands.startSyncOver(folder, passphrase, deviceName);
}

export async function leaveSync(): Promise<Result<null, SyncError>> {
  return await commands.leaveSync();
}

export async function syncNow(): Promise<Result<SyncReportView, PortfolioSyncError>> {
  return await commands.syncNow();
}

export async function pauseSync(): Promise<Result<SyncStatusView, SyncError>> {
  return await commands.pauseSync();
}

export async function resumeSync(): Promise<Result<SyncReportView, PortfolioSyncError>> {
  return await commands.resumeSync();
}

export async function getSyncStatus(): Promise<Result<SyncStatusView, PortfolioSyncError>> {
  return await commands.getSyncStatus();
}

export async function renameSyncDevice(
  deviceName: string,
): Promise<Result<SyncStatusView, SyncError>> {
  return await commands.renameSyncDevice(deviceName);
}

export async function changeSyncFolder(
  folder: string,
): Promise<Result<SyncStatusView, PortfolioSyncError>> {
  return await commands.changeSyncFolder(folder);
}

export async function dismissConflictNotice(noticeId: string): Promise<Result<null, SyncError>> {
  return await commands.dismissConflictNotice(noticeId);
}

/**
 * D11 — the native folder picker only fills the folder field; the chosen path
 * is validated by `inspectSyncFolder` like a typed one. `null` when cancelled.
 */
export async function pickSyncFolder(): Promise<string | null> {
  return await open({ directory: true, multiple: false });
}

// AGT-036 — the owner's setting, and whether this system has an agent connection.
export async function getAgentConnectionState(): Promise<AgentConnectionState> {
  return await commands.getAgentConnectionState();
}

// AGT-022 — the owner allows or stops allowing agents to connect.
export async function setAgentsAllowed(
  allowed: boolean,
): Promise<Result<AgentConnectionState, AgentConnectionError>> {
  return await commands.setAgentsAllowed(allowed);
}

export const settingsGateway = {
  configureScheduledFetch,
  getAgentConnectionState,
  setAgentsAllowed,
  getScheduledFetchStatus,
  inspectSyncFolder,
  enableSync,
  startSyncOver,
  leaveSync,
  syncNow,
  pauseSync,
  resumeSync,
  getSyncStatus,
  renameSyncDevice,
  changeSyncFolder,
  dismissConflictNotice,
  pickSyncFolder,
};

// GPF-011 / FXR-127 — the reference currency, with every code it may be set to. Each feature
// wraps the commands it calls, since features never import each other (F26): the shell and
// currency gateways wrap this read and the rate backfill for their own callers.
export async function getReferenceCurrency(): Promise<Result<ReferenceCurrency, CurrencyError>> {
  return await commands.getReferenceCurrency();
}

// FXR-123/126 — chooses the reference currency.
export async function setReferenceCurrency(currency: string): Promise<Result<null, CurrencyError>> {
  return await commands.setReferenceCurrency(currency);
}

// FXR-015/112 — fetches the rate histories, the new reference currency's pairs included.
export async function backfillCurrencyRateHistory(): Promise<
  Result<number, RateHistoryBackfillError>
> {
  return await commands.backfillCurrencyRateHistory();
}
