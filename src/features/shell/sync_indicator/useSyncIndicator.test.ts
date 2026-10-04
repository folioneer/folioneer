import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SyncStatusView } from "@/bindings";

// Capture the gateway's SyncCompleted callback so the test can fire the event.
let capturedEventListener: (() => void) | null = null;

// 1. Mock the gateway module before importing the hook (test-rules.md § Mocking gateway modules)
vi.mock("../gateway", () => ({
  getSyncStatus: vi.fn(),
  onSyncCompleted: vi.fn((cb: () => void) => {
    capturedEventListener = cb;
    return Promise.resolve(() => {});
  }),
}));

// 2. Import mocked modules for typed access
import * as gateway from "../gateway";
import { useSyncIndicator } from "./useSyncIndicator";

function makeSyncStatus(overrides: Partial<SyncStatusView> = {}): SyncStatusView {
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
    health: "up_to_date",
    ...overrides,
  };
}

describe("useSyncIndicator — visibility (SYN-010/063)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedEventListener = null;
  });

  it("is hidden when sync is disabled", async () => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({
      status: "ok",
      data: makeSyncStatus({ enabled: false }),
    });

    const { result } = renderHook(() => useSyncIndicator());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.visible).toBe(false);
  });

  it("is visible and shows the last-sync time when enabled", async () => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({
      status: "ok",
      data: makeSyncStatus({ enabled: true, last_sync_completed_at: "2026-08-20T10:00:00Z" }),
    });

    const { result } = renderHook(() => useSyncIndicator());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.visible).toBe(true);
    expect(result.current.lastSyncCompletedAt).toBe("2026-08-20T10:00:00Z");
  });
});

describe("useSyncIndicator — attention badge", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedEventListener = null;
  });

  // SYN-063 — the indicator shows the attention badge when the core says sync needs
  // attention, and for nothing else: it reads the health, it does not decide it.
  it.each([
    ["needs_attention", true],
    ["up_to_date", false],
    ["paused", false],
  ] as const)("shows what the core says: %s", async (health, badge) => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({
      status: "ok",
      // A failure is carried in every case: only the health decides.
      data: makeSyncStatus({ failures: ["PortfolioReset"], health }),
    });

    const { result } = renderHook(() => useSyncIndicator());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.needsAttention).toBe(badge);
  });
});

describe("useSyncIndicator — refresh on SyncCompleted (SYN-064)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedEventListener = null;
  });

  it("re-reads sync status when a SyncCompleted event is received", async () => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({ status: "ok", data: makeSyncStatus() });

    renderHook(() => useSyncIndicator());
    await waitFor(() => expect(capturedEventListener).not.toBeNull());

    const callsBefore = vi.mocked(gateway.getSyncStatus).mock.calls.length;

    await act(async () => {
      capturedEventListener?.();
      await Promise.resolve();
    });

    expect(vi.mocked(gateway.getSyncStatus).mock.calls.length).toBeGreaterThan(callsBefore);
  });
});
