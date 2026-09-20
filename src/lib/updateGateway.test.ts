import { beforeEach, describe, expect, it, vi } from "vitest";

const mockCheckForUpdate = vi.hoisted(() => vi.fn());

vi.mock("@/bindings", () => ({ commands: { checkForUpdate: mockCheckForUpdate } }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

import { updateGateway } from "./updateGateway";

describe("updateGateway.checkForUpdate", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // F27 — the gateway hands the typed result through: the hooks branch on it, so a
  // refused check (UPD-028) can never read as "up to date".
  it("hands the available update through", async () => {
    const result = { status: "ok", data: { version: "1.2.3" } };
    mockCheckForUpdate.mockResolvedValue(result);

    await expect(updateGateway.checkForUpdate()).resolves.toBe(result);
  });

  it("hands the typed error through", async () => {
    const result = { status: "error", error: { code: "AccessRefused" } };
    mockCheckForUpdate.mockResolvedValue(result);

    await expect(updateGateway.checkForUpdate()).resolves.toBe(result);
  });
});
