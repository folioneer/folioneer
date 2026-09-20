import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SyncStatus } from "@/bindings";

// Capture the gateway's SyncCompleted callback so the test can fire the event.
let capturedEventListener: (() => void) | null = null;

// 1. Mock the gateway module before importing the hook (test_convention.md § Mocking gateway modules)
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

  it("shows the attention badge when failures are non-empty", async () => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({
      status: "ok",
      data: makeSyncStatus({ failures: ["PortfolioReset"] }),
    });

    const { result } = renderHook(() => useSyncIndicator());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.needsAttention).toBe(true);
  });

  it("shows the attention badge when notices are non-empty", async () => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({
      status: "ok",
      data: makeSyncStatus({
        notices: [
          {
            notice_id: "notice-1",
            kind: "OverruledEdit",
            record_kind: "Transaction",
            record_identity: "tx-1",
            record_label: "Sell 10 AAPL",
            other_device_id: "device-2",
            other_device_name: "Laptop",
            raised_at: "2026-08-20T10:00:00Z",
          },
        ],
      }),
    });

    const { result } = renderHook(() => useSyncIndicator());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.needsAttention).toBe(true);
  });

  it("shows the attention badge when inconsistent holdings are non-empty", async () => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({
      status: "ok",
      data: makeSyncStatus({
        inconsistent_holdings: [
          {
            account_id: "acc-1",
            account_name: "Brokerage",
            asset_id: "asset-1",
            asset_name: "AAPL",
            reason: { Oversold: { quantity: -5_000_000 } },
          },
        ],
      }),
    });

    const { result } = renderHook(() => useSyncIndicator());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.needsAttention).toBe(true);
  });

  it("does not show the attention badge when everything is clean", async () => {
    vi.mocked(gateway.getSyncStatus).mockResolvedValue({ status: "ok", data: makeSyncStatus() });

    const { result } = renderHook(() => useSyncIndicator());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.needsAttention).toBe(false);
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
