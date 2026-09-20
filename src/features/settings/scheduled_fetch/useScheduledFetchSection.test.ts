import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ScheduledFetchRun, ScheduledFetchStatus } from "@/bindings";

// 1. Mock the gateway module before importing the hook (test_convention.md § Mocking gateway modules)
vi.mock("../gateway", () => ({
  configureScheduledFetch: vi.fn(),
  getScheduledFetchStatus: vi.fn(),
}));

// 2. Import mocked modules for typed access
import * as gateway from "../gateway";
import { DEFAULT_TRIGGER_TIME, useScheduledFetchSection } from "./useScheduledFetchSection";

// ---------------------------------------------------------------------------
// Load status on mount — SPF-010/018/052/061
// ---------------------------------------------------------------------------

describe("useScheduledFetchSection — load status on mount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // SPF-018 — local default before the load resolves
  it("defaults triggerTime to 22:15 before the status load resolves (SPF-018)", () => {
    vi.mocked(gateway.getScheduledFetchStatus).mockReturnValue(new Promise(() => {}));

    const { result } = renderHook(() => useScheduledFetchSection());

    expect(result.current.triggerTime).toBe("22:15");
    expect(DEFAULT_TRIGGER_TIME).toBe("22:15");
  });

  // SPF-061 — loading indicator shown while status loads
  it("is loading before the status call resolves (SPF-061)", () => {
    vi.mocked(gateway.getScheduledFetchStatus).mockReturnValue(new Promise(() => {}));

    const { result } = renderHook(() => useScheduledFetchSection());

    expect(result.current.isLoading).toBe(true);
  });

  // SPF-010/052 — configuration and last run loaded into state
  it("loads the configuration and last run into state on mount", async () => {
    const run: ScheduledFetchRun = {
      executed_at: "2026-07-12T19:00:00Z",
      trigger_date: "2026-07-12",
      outcome: "Succeeded",
      updated_count: 12,
      skipped_count: 2,
    };
    const status: ScheduledFetchStatus = {
      configuration: { enabled: true, trigger_time: "19:00" },
      last_run: run,
    };
    vi.mocked(gateway.getScheduledFetchStatus).mockResolvedValue({ status: "ok", data: status });

    const { result } = renderHook(() => useScheduledFetchSection());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.enabled).toBe(true);
    expect(result.current.triggerTime).toBe("19:00");
    expect(result.current.lastRun).toEqual(run);
    expect(result.current.loadError).toBeNull();
  });

  // SPF-061 — inline load error, distinct from configureError
  it("sets loadError to the presented DatabaseError message on load failure (SPF-061)", async () => {
    vi.mocked(gateway.getScheduledFetchStatus).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    const { result } = renderHook(() => useScheduledFetchSection());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.loadError).toEqual({ key: "error.scheduled_fetch.DatabaseError" });
  });
});

// ---------------------------------------------------------------------------
// configure — SPF-012/013/018/024/060
// ---------------------------------------------------------------------------

describe("useScheduledFetchSection — configure", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(gateway.getScheduledFetchStatus).mockResolvedValue({
      status: "ok",
      data: { configuration: { enabled: false, trigger_time: "22:15" }, last_run: null },
    });
  });

  // SPF-060 — in-flight indicator while the configure call is being acknowledged
  it("sets isConfiguring while the configure call is in flight and clears it after (SPF-060)", async () => {
    let resolveConfigure!: (v: { status: "ok"; data: null }) => void;
    vi.mocked(gateway.configureScheduledFetch).mockReturnValue(
      new Promise((resolve) => {
        resolveConfigure = resolve;
      }),
    );

    const { result } = renderHook(() => useScheduledFetchSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => {
      void result.current.configure(true, "19:00");
    });
    expect(result.current.isConfiguring).toBe(true);

    await act(async () => resolveConfigure({ status: "ok", data: null }));
    expect(result.current.isConfiguring).toBe(false);
  });

  // SPF-012/024 — success updates state and re-reads status (no live event bridge)
  it("updates enabled/triggerTime and re-reads status after a successful configure (SPF-024)", async () => {
    vi.mocked(gateway.configureScheduledFetch).mockResolvedValue({ status: "ok", data: null });

    const { result } = renderHook(() => useScheduledFetchSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.configure(true, "19:00");
    });

    expect(result.current.enabled).toBe(true);
    expect(result.current.triggerTime).toBe("19:00");
    expect(gateway.configureScheduledFetch).toHaveBeenCalledWith(true, "19:00");
    expect(gateway.getScheduledFetchStatus).toHaveBeenCalledTimes(2); // mount + post-configure refresh
  });

  // SPF-013 — toggle/time revert to their prior values on rejection
  it("reverts enabled and triggerTime to their prior values when configure fails (SPF-013)", async () => {
    vi.mocked(gateway.configureScheduledFetch).mockResolvedValue({
      status: "error",
      error: { code: "ScheduleRegistrationFailed" },
    });

    const { result } = renderHook(() => useScheduledFetchSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.enabled).toBe(false);

    await act(async () => {
      await result.current.configure(true, "19:00");
    });

    expect(result.current.enabled).toBe(false);
    expect(result.current.triggerTime).toBe("22:15");
    expect(result.current.configureError).toEqual({
      key: "error.scheduled_fetch.ScheduleRegistrationFailed",
    });
  });

  // SPF-013 — a prior configureError clears on the next successful configure
  it("clears a prior configureError on a new successful configure call", async () => {
    vi.mocked(gateway.configureScheduledFetch)
      .mockResolvedValueOnce({ status: "error", error: { code: "InvalidTriggerTime" } })
      .mockResolvedValueOnce({ status: "ok", data: null });

    const { result } = renderHook(() => useScheduledFetchSection());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.configure(true, "25:99");
    });
    expect(result.current.configureError).not.toBeNull();

    await act(async () => {
      await result.current.configure(true, "19:00");
    });
    expect(result.current.configureError).toBeNull();
  });
});
