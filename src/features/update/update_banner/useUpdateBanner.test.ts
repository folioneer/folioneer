import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { UpdateError } from "@/bindings";

// --- mocks ---

const mockCheckForUpdate = vi.fn().mockResolvedValue(null);
const mockDownloadUpdate = vi.fn().mockResolvedValue(undefined);
const mockInstallUpdate = vi.fn().mockResolvedValue(undefined);

type AvailableCallback = (info: { version: string }) => void;
type CompleteCallback = () => void;
type ErrorCallback = (error: UpdateError) => void;

let onAvailableCb: AvailableCallback | null = null;
let onCompleteCb: CompleteCallback | null = null;
let onErrorCb: ErrorCallback | null = null;

vi.mock("@/lib/updateGateway", () => ({
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
});
