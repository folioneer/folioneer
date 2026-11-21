import { useCallback, useEffect, useRef, useState } from "react";
import { logger } from "@/lib/logger";
import type { UpdateError } from "../gateway";
import { updateGateway } from "../gateway";
import { isRetryable, presentUpdateError } from "../shared/presenter";

const GENERIC_ERROR_KEY = "update.error";

export type UpdateBannerState = "idle" | "available" | "downloading" | "ready" | "error";

export interface UpdateBannerData {
  state: UpdateBannerState;
  /** i18n key of the message shown in the error state (UPD-023, #873's mock). */
  errorKey: string;
  /** Whether the error state offers "Retry" (UPD-023) or "Dismiss" (#873's mock). */
  canRetry: boolean;
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
  const [errorKey, setErrorKey] = useState(GENERIC_ERROR_KEY);
  const [canRetry, setCanRetry] = useState(true);

  const showError = useCallback((error: UpdateError | null) => {
    setErrorKey(error ? presentUpdateError(error) : GENERIC_ERROR_KEY);
    setCanRetry(error ? isRetryable(error) : true);
    setState("error");
  }, []);
  const [version, setVersion] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [isRestarting, setIsRestarting] = useState(false);

  // UPD-012 — the events below read the state without re-subscribing.
  const stateRef = useRef<UpdateBannerState>(state);
  useEffect(() => {
    stateRef.current = state;
  }, [state]);

  useEffect(() => {
    logger.info("[UpdateBanner] mounted — starting update check (R1)");

    let mounted = true;

    // Startup check (R1): triggered after interface is loaded
    updateGateway
      .checkForUpdate()
      .then((result) => {
        // UPD-021 — only a refusal shows (#873's mock); any other failure stays silent.
        if (mounted && result.status === "error" && !isRetryable(result.error)) {
          showError(result.error);
        }
      })
      .catch((e) => {
        logger.error("[UpdateBanner] Startup check failed", e);
      });

    // Listen for update:available (R3) — emitted by backend on check; a manual check shows
    // a version dismissed earlier again (#873's mock).
    const unlistenAvailable = updateGateway.onUpdateAvailable((info) => {
      if (!mounted) return;
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
      // Read at once by the error listener, before the effect catches up (UPD-012).
      stateRef.current = "ready";
      setState("ready");
      setProgress(100);
    });

    // Listen for update errors: a failed download (R23), or an access the update
    // server refuses, announced by any check (UPD-028) — which leaves "Ready to install"
    // as it is (UPD-012, #873's mock).
    const unlistenError = updateGateway.onUpdateError((error) => {
      if (!mounted || stateRef.current === "ready") return;
      showError(error);
    });

    return () => {
      mounted = false;
      unlistenAvailable.then((fn) => fn()).catch(() => {});
      unlistenProgress.then((fn) => fn()).catch(() => {});
      unlistenComplete.then((fn) => fn()).catch(() => {});
      unlistenError.then((fn) => fn()).catch(() => {});
    };
  }, [showError]);

  // R6 — start download
  const handleInstall = useCallback(() => {
    setState("downloading");
    setProgress(0);
    updateGateway.downloadUpdate().catch((e) => {
      logger.error("[UpdateBanner] downloadUpdate command failed", e);
      showError(null);
    });
  }, [showError]);

  // R5 — dismiss: allowed in 'available' state and for a refused access (#873's mock); no-op in 'ready' (R12)
  const handleDismiss = useCallback(() => {
    const refused = state === "error" && !canRetry;
    if (state !== "available" && !refused) return;
    setState("idle");
  }, [state, canRetry]);

  // R24 — retry download from scratch
  const handleRetry = useCallback(() => {
    setState("downloading");
    setProgress(0);
    updateGateway.downloadUpdate().catch((e) => {
      logger.error("[UpdateBanner] downloadUpdate retry failed", e);
      showError(null);
    });
  }, [showError]);

  // R13 — install and restart; guard against double-click. UPD-013A / UPD-023 — a refused
  // install shows the error state and gives the button back.
  const handleRestart = useCallback(async () => {
    if (isRestarting) return;
    setIsRestarting(true);
    try {
      const result = await updateGateway.installUpdate();
      if (result.status === "error") {
        logger.error("[UpdateBanner] installUpdate refused", { error: result.error });
        setIsRestarting(false);
        showError(result.error);
      }
    } catch (e) {
      logger.error("[UpdateBanner] installUpdate failed", e);
      setIsRestarting(false);
      showError(null);
    }
  }, [isRestarting, showError]);

  return {
    state,
    errorKey,
    canRetry,
    version,
    progress,
    isRestarting,
    handleInstall,
    handleDismiss,
    handleRetry,
    handleRestart,
  };
}
