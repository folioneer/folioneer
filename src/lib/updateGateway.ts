import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type Result, type UpdateError, type UpdateInfo } from "@/bindings";

export type { UpdateError, UpdateInfo };

export const updateGateway = {
  async checkForUpdate(): Promise<Result<UpdateInfo | null, UpdateError>> {
    return await commands.checkForUpdate();
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
