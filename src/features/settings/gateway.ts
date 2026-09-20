import { open } from "@tauri-apps/plugin-dialog";
import {
  commands,
  type PortfolioSyncError,
  type Result,
  type ScheduledFetchError,
  type ScheduledFetchStatus,
  type SyncError,
  type SyncFolderState,
  type SyncReport,
  type SyncStatus,
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
): Promise<Result<SyncStatus, PortfolioSyncError>> {
  return await commands.enableSync(folder, passphrase, deviceName);
}

export async function startSyncOver(
  folder: string,
  passphrase: string,
  deviceName: string,
): Promise<Result<SyncStatus, PortfolioSyncError>> {
  return await commands.startSyncOver(folder, passphrase, deviceName);
}

export async function leaveSync(): Promise<Result<null, SyncError>> {
  return await commands.leaveSync();
}

export async function syncNow(): Promise<Result<SyncReport, PortfolioSyncError>> {
  return await commands.syncNow();
}

export async function pauseSync(): Promise<Result<SyncStatus, SyncError>> {
  return await commands.pauseSync();
}

export async function resumeSync(): Promise<Result<SyncReport, PortfolioSyncError>> {
  return await commands.resumeSync();
}

export async function getSyncStatus(): Promise<Result<SyncStatus, PortfolioSyncError>> {
  return await commands.getSyncStatus();
}

export async function renameSyncDevice(deviceName: string): Promise<Result<SyncStatus, SyncError>> {
  return await commands.renameSyncDevice(deviceName);
}

export async function changeSyncFolder(
  folder: string,
): Promise<Result<SyncStatus, PortfolioSyncError>> {
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

export const settingsGateway = {
  configureScheduledFetch,
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
