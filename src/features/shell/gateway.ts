import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Event,
  FeeGenerationError,
  PortfolioSyncError,
  PriceFreshness,
  PriceFreshnessError,
  Result,
  SyncStatus,
} from "@/bindings";
import { commands, events } from "@/bindings";

// SYN-063 — the shell indicator reads the sync status through its own gateway (F26).
export function getSyncStatus(): Promise<Result<SyncStatus, PortfolioSyncError>> {
  return commands.getSyncStatus();
}

// SYN-064 — a run that applied changes or changed the device's state has completed.
export function onSyncCompleted(callback: () => void): Promise<UnlistenFn> {
  return events.event.listen((event) => {
    if (event.payload.type === "SyncCompleted") {
      callback();
    }
  });
}

// MKT-202 — the header's price item reads its two figures through the shell's gateway (F26).
export function getPriceFreshness(): Promise<Result<PriceFreshness, PriceFreshnessError>> {
  return commands.getPriceFreshness();
}

// MKT-203 — the kind of every backend event, for the price item to pick what concerns it.
export function subscribeToEvents(callback: (type: Event["type"]) => void): Promise<UnlistenFn> {
  return events.event.listen((event) => callback(event.payload.type));
}

export const shellGateway = {
  onMigrationError(cb: (message: string) => void): Promise<UnlistenFn> {
    return listen<string>("db:migration_error", (event) => cb(event.payload));
  },

  // FEE-040 — apply every due recurring fee deduction (lazy catch-up on app start).
  applyDueFeeDeductions(): Promise<Result<null, FeeGenerationError>> {
    return commands.applyDueFeeDeductions();
  },

  getSyncStatus,
  onSyncCompleted,
  getPriceFreshness,
  subscribeToEvents,
};
