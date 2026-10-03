import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { messageText } from "@/ui/format/i18n";
import { useSyncSummary } from "./useSyncSummary";

/**
 * SYN-063 — what the settings keep of sync: whether it is enabled or paused on this
 * computer, and the way to its page.
 */
export function SyncSummary() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { stateKey, loadError } = useSyncSummary();

  return (
    <section className="flex flex-col gap-3">
      <span className="text-sm font-medium text-m3-on-surface-variant">{t("sync.title")}</span>
      <div className="flex items-center justify-between gap-4">
        {loadError ? (
          <span id="settings-sync-error" role="alert" className="text-sm text-m3-error">
            {messageText(t, loadError)}
          </span>
        ) : (
          <span id="settings-sync-state" className="text-sm text-m3-on-surface">
            {stateKey ? t(stateKey) : null}
          </span>
        )}
        <Button
          id="settings-open-sync"
          data-testid="settings-open-sync"
          variant="outline"
          size="sm"
          onClick={() => void navigate({ to: "/sync" })}
        >
          {t("sync.open_page")}
        </Button>
      </div>
    </section>
  );
}
