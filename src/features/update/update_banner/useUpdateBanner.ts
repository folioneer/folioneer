import { useCallback, useEffect, useRef, useState } from "react";
import { logger } from "@/lib/logger";
import { updateGateway } from "@/lib/updateGateway";

export type UpdateBannerState = "idle" | "available" | "downloading" | "ready" | "error";

export interface UpdateBannerData {
  state: UpdateBannerState;
  version: string | null;
  progress: number;
  isRestarting: boolean;
  handleInstall: () => void;
  handleDismiss: () => void;
  handleRetry: () => void;
  handleRestart: () => Promise<void>;
}

export function useUpdateBanner(): UpdateBannerData {
  const [state, setState] = useState<UpdateBannerState>("idle");
  const [version, setVersion] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [isRestarting, setIsRestarting] = useState(false);

  // Track if dismissed so we can ignore a re-emitted update:available in same session
  const dismissedVersion = useRef<string | null>(null);

  useEffect(() => {
    logger.info("[UpdateBanner] mounted — starting update check (R1)");

    let mounted = true;

    // Startup check (R1): triggered after interface is loaded
    updateGateway.checkForUpdate().catch((e) => {
      logger.error("[UpdateBanner] Startup check failed", e);
    });

    // Listen for update:available (R3) — emitted by backend on check
    const unlistenAvailable = updateGateway.onUpdateAvailable((info) => {
      if (!mounted) return;
      if (dismissedVersion.current === info.version) return;
      setState((prev) => {
        if (prev === "idle") {
          setVersion(info.version);
          return "available";
        }
        return prev;
      });
    });

    // Listen for download progress (R8)
    const unlistenProgress = updateGateway.onUpdateProgress((percent) => {
      if (!mounted) return;
      setProgress(percent);
    });

    // Listen for download complete (R11)
    const unlistenComplete = updateGateway.onUpdateComplete(() => {
      if (!mounted) return;
      setState("ready");
      setProgress(100);
    });

    // Listen for download error (R23). The typed UpdateError variant is logged
    // server-side; the banner shows a single generic message, so the payload is
    // not surfaced to the user.
    const unlistenError = updateGateway.onUpdateError(() => {
      if (!mounted) return;
      setState("error");
    });

    return () => {
      mounted = false;
      unlistenAvailable.then((fn) => fn()).catch(() => {});
      unlistenProgress.then((fn) => fn()).catch(() => {});
      unlistenComplete.then((fn) => fn()).catch(() => {});
      unlistenError.then((fn) => fn()).catch(() => {});
    };
  }, []);

  // R6 — start download
  const handleInstall = useCallback(() => {
    setState("downloading");
    setProgress(0);
    updateGateway.downloadUpdate().catch((e) => {
      logger.error("[UpdateBanner] downloadUpdate command failed", e);
      setState("error");
    });
  }, []);

  // R5 — dismiss: only allowed in 'available' state; no-op in 'ready' (R12)
  const handleDismiss = useCallback(() => {
    if (state !== "available") return;
    dismissedVersion.current = version;
    setState("idle");
  }, [state, version]);

  // R24 — retry download from scratch
  const handleRetry = useCallback(() => {
    setState("downloading");
    setProgress(0);
    updateGateway.downloadUpdate().catch((e) => {
      logger.error("[UpdateBanner] downloadUpdate retry failed", e);
      setState("error");
    });
  }, []);

  // R13 — install and restart; guard against double-click
  const handleRestart = useCallback(async () => {
    if (isRestarting) return;
    setIsRestarting(true);
    try {
      await updateGateway.installUpdate();
    } catch (e) {
      logger.error("[UpdateBanner] installUpdate failed", e);
      setIsRestarting(false);
    }
  }, [isRestarting]);

  return {
    state,
    version,
    progress,
    isRestarting,
    handleInstall,
    handleDismiss,
    handleRetry,
    handleRestart,
  };
}
