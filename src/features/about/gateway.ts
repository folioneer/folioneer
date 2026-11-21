import { commands, type Result, type UpdateError, type UpdateInfo } from "@/bindings";

/** F3 — the About page's own way to the core: the manual update check (UPD-025). */
export const aboutGateway = {
  async checkForUpdate(): Promise<Result<UpdateInfo | null, UpdateError>> {
    return await commands.checkForUpdate();
  },
};
