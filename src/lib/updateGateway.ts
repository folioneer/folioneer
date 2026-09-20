import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type UpdateError, type UpdateInfo } from "@/bindings";
import { logger } from "@/lib/logger";

export type { UpdateError, UpdateInfo };

export const updateGateway = {
  async checkForUpdate(): Promise<UpdateInfo | null> {
    const result = await commands.checkForUpdate();
    if (result.status === "ok") return result.data;
    logger.error("[update] checkForUpdate failed", result.error);
    return null;
  },

  async downloadUpdate(): Promise<void> {
    await commands.downloadUpdate();
  },

  async installUpdate(): Promise<void> {
    await commands.installUpdate();
  },

  onUpdateAvailable(cb: (info: UpdateInfo) => void): Promise<UnlistenFn> {
    return listen<UpdateInfo>("update:available", (event) => cb(event.payload));
  },

  onUpdateProgress(cb: (percent: number) => void): Promise<UnlistenFn> {
    return listen<number>("update:progress", (event) => cb(event.payload));
  },

  onUpdateComplete(cb: () => void): Promise<UnlistenFn> {
    return listen<null>("update:complete", () => cb());
  },

  onUpdateError(cb: (error: UpdateError) => void): Promise<UnlistenFn> {
    return listen<UpdateError>("update:error", (event) => cb(event.payload));
  },
};
