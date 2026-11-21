import { beforeEach, describe, expect, it, vi } from "vitest";

const mockCheckForUpdate = vi.hoisted(() => vi.fn());

vi.mock("@/bindings", () => ({ commands: { checkForUpdate: mockCheckForUpdate } }));
vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

import { aboutGateway } from "./gateway";

describe("aboutGateway.checkForUpdate", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // F27 — the gateway hands the typed result through: the hooks branch on it, so a
  // refused check (UPD-028) can never read as "up to date".
  it("hands the available update through", async () => {
    const result = { status: "ok", data: { version: "1.2.3" } };
    mockCheckForUpdate.mockResolvedValue(result);

    await expect(aboutGateway.checkForUpdate()).resolves.toBe(result);
  });

  it("hands the typed error through", async () => {
    const result = { status: "error", error: { code: "AccessRefused" } };
    mockCheckForUpdate.mockResolvedValue(result);

    await expect(aboutGateway.checkForUpdate()).resolves.toBe(result);
  });
});
