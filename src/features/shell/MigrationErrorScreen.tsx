import { useTranslation } from "react-i18next";
import type { MigrationFailure } from "@/bindings";

interface MigrationErrorScreenProps {
  failure: MigrationFailure;
}

/**
 * UPD-018 / UPD-035 — the application blocked at startup. After a failed migration whose
 * copy was put back, it says nothing was lost and names the folder of the copies; any other
 * failure gets the plain message. The error is shown either way.
 */
export function MigrationErrorScreen({ failure }: MigrationErrorScreenProps) {
  const { t } = useTranslation();
  const restored = failure.backups_dir !== null;

  return (
    <div className="h-screen flex items-center justify-center bg-m3-surface p-8">
      <div className="max-w-md text-center flex flex-col gap-4">
        <p id="migration-error-message" className="text-m3-error font-medium text-lg">
          {restored ? t("app.migration_restored") : t("app.migration_error")}
        </p>
        {restored && (
          <div className="flex flex-col gap-1 text-sm text-m3-on-surface">
            <p>{t("app.migration_backups")}</p>
            <p id="migration-error-backups" className="font-mono break-all">
              {failure.backups_dir}
            </p>
          </div>
        )}
        <p id="migration-error-detail" className="text-m3-on-surface-variant text-sm font-mono">
          {failure.detail}
        </p>
      </div>
    </div>
  );
}
