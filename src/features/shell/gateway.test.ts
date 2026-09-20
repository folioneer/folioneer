import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SyncStatus } from "@/bindings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockInvoke = vi.mocked(invoke);

// Import after mock is registered so bindings.ts picks up the mock
const { shellGateway } = await import("./gateway");

function makeSyncStatus(overrides: Partial<SyncStatus> = {}): SyncStatus {
  return {
    enabled: true,
    paused: false,
    device_id: "device-1",
    device_name: "Desktop",
    folder: "/home/user/sync",
    app_version: "0.43.0",
    last_sync_completed_at: "2026-08-20T10:00:00Z",
    roster: [],
    held_back_count: 0,
    oldest_held_back_since: null,
    notices: [],
    inconsistent_holdings: [],
    failures: [],
    ...overrides,
  };
}

// shellGateway owns its own gateway (divergence #13 precedent) so the shell's
// sync indicator does not import across features (F26).
describe("shellGateway — getSyncStatus (SYN-063)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("getSyncStatus passes through ok result with no args", async () => {
    const status = makeSyncStatus();
    mockInvoke.mockResolvedValue(status);

    const result = await shellGateway.getSyncStatus();

    expect(result).toEqual({ status: "ok", data: status });
    expect(mockInvoke).toHaveBeenCalledWith("get_sync_status");
  });

  it("getSyncStatus passes through DatabaseError", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await shellGateway.getSyncStatus();

    expect(result).toEqual({ status: "error", error: { code: "DatabaseError" } });
  });
});

describe("shellGateway — getPriceFreshness (MKT-202)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("getPriceFreshness passes through ok result with no args", async () => {
    const freshness = { newest_price_date: "2026-09-15", last_fetch_at: null };
    mockInvoke.mockResolvedValue(freshness);

    const result = await shellGateway.getPriceFreshness();

    expect(result).toEqual({ status: "ok", data: freshness });
    expect(mockInvoke).toHaveBeenCalledWith("get_price_freshness");
  });

  it("getPriceFreshness passes through DatabaseError", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await shellGateway.getPriceFreshness();

    expect(result).toEqual({ status: "error", error: { code: "DatabaseError" } });
  });
});
