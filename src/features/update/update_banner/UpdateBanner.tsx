import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, IconButton } from "@/ui/components";
import type { UpdateBannerData } from "./useUpdateBanner";

interface UpdateBannerProps {
  data: UpdateBannerData;
}

// R2 — banner is not rendered when idle
// R4 — banner is part of shell layout, visible on all pages
export function UpdateBanner({ data }: UpdateBannerProps) {
  const { t } = useTranslation();
  const {
    state,
    version,
    progress,
    isRestarting,
    handleInstall,
    handleDismiss,
    handleRetry,
    handleRestart,
  } = data;

  if (state === "idle") return null;

  return (
    <div
      className="w-full bg-m3-secondary-container text-m3-on-secondary-container px-4 py-2 flex min-h-[48px] items-center justify-between gap-4 text-sm"
      role="status"
      aria-live="polite"
    >
      {/* R3 — available state */}
      {state === "available" && (
        <>
          <span>{t("update.available", { version })}</span>
          <div className="flex items-center gap-2">
            <Button size="sm" variant="primary" onClick={handleInstall}>
              {t("update.action_install")}
            </Button>
            <Button size="sm" variant="ghost" onClick={handleDismiss}>
              {t("update.action_dismiss")}
            </Button>
            {/* R5 — × has same effect as Ignorer */}
            <IconButton
              size="sm"
              variant="ghost"
              icon={<X size={16} />}
              aria-label={t("action.close")}
              onClick={handleDismiss}
            />
          </div>
        </>
      )}

      {/* R8 — downloading state with progress */}
      {state === "downloading" && (
        <>
          <span>{t("update.downloading")}</span>
          <div className="flex items-center gap-3">
            <div
              className="w-32 h-1.5 bg-m3-outline/30 rounded-full overflow-hidden"
              role="progressbar"
              aria-valuenow={progress}
              aria-valuemin={0}
              aria-valuemax={100}
            >
              <div
                className="h-full bg-m3-primary rounded-full transition-all"
                style={{ width: `${progress}%` }}
              />
            </div>
            <span className="text-xs text-m3-on-secondary-container/70">
              {t("update.progress_label", { percent: progress })}
            </span>
          </div>
        </>
      )}

      {/* R11 — ready state, R12 — no × or Ignorer */}
      {state === "ready" && (
        <>
          <span className="font-medium">{t("update.ready")}</span>
          <Button
            size="sm"
            variant="primary"
            loading={isRestarting}
            disabled={isRestarting}
            onClick={() => void handleRestart()}
          >
            {t("update.action_restart")}
          </Button>
        </>
      )}

      {/* R23 — error state with retry */}
      {state === "error" && (
        <>
          <span className="text-m3-error">{t("update.error")}</span>
          <Button size="sm" variant="primary" onClick={handleRetry}>
            {t("update.action_retry")}
          </Button>
        </>
      )}
    </div>
  );
}
