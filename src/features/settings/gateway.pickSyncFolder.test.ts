import { beforeEach, describe, expect, it, vi } from "vitest";

// D11 — the folder picker: `pickSyncFolder()` wraps
// `open({ directory: true, multiple: false })` from `@tauri-apps/plugin-dialog`.
// Isolated in its own file so a missing-package resolution failure (the npm
// dependency is not yet installed — only the Rust `tauri-plugin-dialog` crate +
// capability landed in PR-B; adding `@tauri-apps/plugin-dialog` to package.json
// is the implementer's job) does not collapse the other gateway.test.ts cases.
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const { open } = await import("@tauri-apps/plugin-dialog");
const mockOpen = vi.mocked(open);

const { settingsGateway } = await import("./gateway");

describe("settingsGateway — pickSyncFolder (D11)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("opens a directory-only, single-selection dialog and returns the chosen path", async () => {
    mockOpen.mockResolvedValue("/home/user/chosen-sync-folder");

    const result = await settingsGateway.pickSyncFolder();

    expect(result).toBe("/home/user/chosen-sync-folder");
    expect(mockOpen).toHaveBeenCalledWith({ directory: true, multiple: false });
  });

  it("returns null when the user cancels the dialog", async () => {
    mockOpen.mockResolvedValue(null);

    const result = await settingsGateway.pickSyncFolder();

    expect(result).toBeNull();
  });
});
