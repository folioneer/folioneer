import { AlertCircle, CheckCircle, Info, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { type SnackbarVariant, useSnackbarStore } from "@/ui/components/snackbar/snackbarStore";

const ICONS: Record<SnackbarVariant, React.ElementType> = {
  success: CheckCircle,
  error: AlertCircle,
  info: Info,
};

const STYLES: Record<SnackbarVariant, string> = {
  success: "bg-m3-primary text-m3-on-primary",
  error: "bg-m3-error text-m3-on-error",
  info: "bg-m3-surface-container-highest text-m3-on-surface",
};

const HOVER: Record<SnackbarVariant, string> = {
  success: "hover:bg-m3-on-primary/10",
  error: "hover:bg-m3-on-error/10",
  info: "hover:bg-m3-on-surface/10",
};

export function Snackbar() {
  const { t } = useTranslation("common");
  const { message, variant, isVisible, hide } = useSnackbarStore();

  return (
    <div role="status" aria-live="polite" aria-atomic="true">
      {isVisible && (
        <div
          className={`
            fixed bottom-6 left-1/2 -translate-x-1/2 z-[200]
            flex items-center gap-3 px-4 py-3 rounded-2xl
            shadow-elevation-3 animate-in slide-in-from-bottom-4 fade-in duration-200
            min-w-64 max-w-sm
            ${STYLES[variant]}
          `}
        >
          {(() => {
            const Icon = ICONS[variant];
            return <Icon size={18} className="shrink-0" />;
          })()}
          <span className="flex-1 text-sm font-medium">{message}</span>
          <button
            type="button"
            onClick={hide}
            aria-label={t("action.dismiss")}
            className={`shrink-0 p-1 rounded-full transition-colors ${HOVER[variant]}`}
          >
            <X size={16} />
          </button>
        </div>
      )}
    </div>
  );
}
