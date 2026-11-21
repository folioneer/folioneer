import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { UpdateError } from "@/bindings";

// --- mocks ---

const mockCheckForUpdate = vi.fn().mockResolvedValue({ status: "ok", data: null });
const mockDownloadUpdate = vi.fn().mockResolvedValue(undefined);
const mockInstallUpdate = vi.fn().mockResolvedValue({ status: "ok", data: null });

type AvailableCallback = (info: { version: string }) => void;
type CompleteCallback = () => void;
type ErrorCallback = (error: UpdateError) => void;

let onAvailableCb: AvailableCallback | null = null;
let onCompleteCb: CompleteCallback | null = null;
let onErrorCb: ErrorCallback | null = null;

vi.mock("../gateway", () => ({
  updateGateway: {
    checkForUpdate: () => mockCheckForUpdate(),
    downloadUpdate: () => mockDownloadUpdate(),
    installUpdate: () => mockInstallUpdate(),
    onUpdateAvailable: (cb: AvailableCallback) => {
      onAvailableCb = cb;
      return Promise.resolve(() => {
        onAvailableCb = null;
      });
    },
    onUpdateProgress: () => Promise.resolve(() => {}),
    onUpdateComplete: (cb: CompleteCallback) => {
      onCompleteCb = cb;
      return Promise.resolve(() => {
        onCompleteCb = null;
      });
    },
    onUpdateError: (cb: ErrorCallback) => {
      onErrorCb = cb;
      return Promise.resolve(() => {
        onErrorCb = null;
      });
    },
  },
}));

vi.mock("@/lib/logger", () => ({
  logger: { info: vi.fn(), error: vi.fn() },
}));

import { useUpdateBanner } from "./useUpdateBanner";

describe("useUpdateBanner", () => {
  beforeEach(() => {
    onAvailableCb = null;
    onCompleteCb = null;
    onErrorCb = null;
    mockDownloadUpdate.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // R3 — idle → available on update:available event
  it("transitions idle → available when update:available is received", async () => {
    const { result } = renderHook(() => useUpdateBanner());
    expect(result.current.state).toBe("idle");

    await act(async () => {
      onAvailableCb?.({ version: "1.2.3" });
    });

    expect(result.current.state).toBe("available");
    expect(result.current.version).toBe("1.2.3");
  });

  // R6 — available → downloading on handleInstall
  it("transitions available → downloading on handleInstall", async () => {
    const { result } = renderHook(() => useUpdateBanner());

    await act(async () => {
      onAvailableCb?.({ version: "1.2.3" });
    });
    act(() => {
      result.current.handleInstall();
    });

    expect(result.current.state).toBe("downloading");
    expect(mockDownloadUpdate).toHaveBeenCalledTimes(1);
  });

  // R11 — downloading → ready on update:complete
  it("transitions downloading → ready when update:complete is received", async () => {
    const { result } = renderHook(() => useUpdateBanner());

    await act(async () => {
      onAvailableCb?.({ version: "1.2.3" });
    });
    act(() => {
      result.current.handleInstall();
    });
    act(() => {
      onCompleteCb?.();
    });

    expect(result.current.state).toBe("ready");
    expect(result.current.progress).toBe(100);
  });

  // R23 — downloading → error on update:error
  it("transitions downloading → error when update:error is received", async () => {
    const { result } = renderHook(() => useUpdateBanner());

    await act(async () => {
      onAvailableCb?.({ version: "1.2.3" });
    });
    act(() => {
      result.current.handleInstall();
    });
    act(() => {
      onErrorCb?.({ code: "OperationFailed" });
    });

    expect(result.current.state).toBe("error");
  });

  // UPD-028 — a refused access is announced at startup too, with no download running,
  // and the banner names it instead of the generic download failure.
  it("shows the refused-access message when update:error carries AccessRefused", async () => {
    const { result } = renderHook(() => useUpdateBanner());

    act(() => {
      onErrorCb?.({ code: "AccessRefused" });
    });

    expect(result.current.state).toBe("error");
    expect(result.current.errorKey).toBe("update.error_access_refused");
    expect(result.current.canRetry).toBe(false);
  });

  // #873's mock — the startup check's own answer is enough: the refusal shows without
  // waiting for an event.
  it("shows the refusal when the startup check itself is refused", async () => {
    mockCheckForUpdate.mockResolvedValueOnce({ status: "error", error: { code: "AccessRefused" } });

    const { result } = renderHook(() => useUpdateBanner());

    await waitFor(() => expect(result.current.state).toBe("error"));
    expect(result.current.errorKey).toBe("update.error_access_refused");
  });

  // #873's mock — a refused access is dismissed for the session; a download failure is not dismissible.
  it("dismisses a refused access but not a download failure", async () => {
    const { result } = renderHook(() => useUpdateBanner());

    act(() => {
      onErrorCb?.({ code: "OperationFailed" });
    });
    act(() => {
      result.current.handleDismiss();
    });
    expect(result.current.state).toBe("error");

    act(() => {
      onErrorCb?.({ code: "AccessRefused" });
    });
    act(() => {
      result.current.handleDismiss();
    });
    expect(result.current.state).toBe("idle");
  });

  it("shows the generic message for any other update:error", async () => {
    const { result } = renderHook(() => useUpdateBanner());

    act(() => {
      onErrorCb?.({ code: "OperationFailed" });
    });

    expect(result.current.errorKey).toBe("update.error");
    expect(result.current.canRetry).toBe(true);
  });

  // R24 — error → downloading on handleRetry
  it("transitions error → downloading on handleRetry", async () => {
    const { result } = renderHook(() => useUpdateBanner());

    await act(async () => {
      onAvailableCb?.({ version: "1.2.3" });
    });
    act(() => {
      result.current.handleInstall();
    });
    act(() => {
      onErrorCb?.({ code: "OperationFailed" });
    });
    act(() => {
      result.current.handleRetry();
    });

    expect(result.current.state).toBe("downloading");
    expect(mockDownloadUpdate).toHaveBeenCalledTimes(2);
  });

  // R12 — handleDismiss is a no-op in 'ready' state
  it("handleDismiss is no-op in ready state (R12)", async () => {
    const { result } = renderHook(() => useUpdateBanner());

    await act(async () => {
      onAvailableCb?.({ version: "1.2.3" });
    });
    act(() => {
      result.current.handleInstall();
    });
    act(() => {
      onCompleteCb?.();
    });

    expect(result.current.state).toBe("ready");

    act(() => {
      result.current.handleDismiss();
    });

    expect(result.current.state).toBe("ready");
  });

  // UPD-013A (#698) — an install the core refuses shows its error and gives the button back,
  // instead of spinning forever.
  it("shows the error and stops restarting when the install is refused", async () => {
    mockInstallUpdate.mockResolvedValue({ status: "error", error: { code: "OperationFailed" } });
    const { result } = renderHook(() => useUpdateBanner());
    await act(async () => {
      onAvailableCb?.({ version: "1.2.3" });
    });
    act(() => {
      result.current.handleInstall();
    });
    act(() => {
      onCompleteCb?.();
    });

    await act(async () => {
      await result.current.handleRestart();
    });

    expect(result.current.isRestarting).toBe(false);
    expect(result.current.state).toBe("error");
    expect(result.current.errorKey).toBe("update.error");
    expect(result.current.canRetry).toBe(true);
  });

  // UPD-021 — a startup check that fails for any reason but a refusal shows nothing.
  it("stays silent when the startup check fails without a refusal", async () => {
    mockCheckForUpdate.mockResolvedValueOnce({
      status: "error",
      error: { code: "OperationFailed" },
    });

    const { result } = renderHook(() => useUpdateBanner());

    await waitFor(() => expect(mockCheckForUpdate).toHaveBeenCalled());
    await act(async () => {});
    expect(result.current.state).toBe("idle");
  });

  // #873's mock — a manual check that finds a version dismissed earlier shows the banner again.
  it("shows the banner again when a check finds a version dismissed earlier", async () => {
    const { result } = renderHook(() => useUpdateBanner());
    await act(async () => {
      onAvailableCb?.({ version: "1.2.3" });
    });
    act(() => {
      result.current.handleDismiss();
    });
    expect(result.current.state).toBe("idle");

    await act(async () => {
      onAvailableCb?.({ version: "1.2.3" });
    });

    expect(result.current.state).toBe("available");
    expect(result.current.version).toBe("1.2.3");
  });

  // #873's mock / UPD-012 — a refusal a check finds while "Ready to install" leaves the banner.
  it("keeps the ready banner when a check is refused", async () => {
    const { result } = renderHook(() => useUpdateBanner());
    await act(async () => {
      onAvailableCb?.({ version: "1.2.3" });
    });
    act(() => {
      result.current.handleInstall();
    });
    act(() => {
      onCompleteCb?.();
    });

    act(() => {
      onErrorCb?.({ code: "AccessRefused" });
    });

    expect(result.current.state).toBe("ready");
  });

  // #873's mock / UPD-028 — an installation refused shows the refusal, with Dismiss only.
  it("shows the refusal when the installation is refused", async () => {
    mockInstallUpdate.mockResolvedValueOnce({ status: "error", error: { code: "AccessRefused" } });
    const { result } = renderHook(() => useUpdateBanner());
    await act(async () => {
      onAvailableCb?.({ version: "1.2.3" });
    });
    act(() => {
      result.current.handleInstall();
    });
    act(() => {
      onCompleteCb?.();
    });

    await act(async () => {
      await result.current.handleRestart();
    });

    expect(result.current.state).toBe("error");
    expect(result.current.errorKey).toBe("update.error_access_refused");
    expect(result.current.canRetry).toBe(false);
  });
});
