import { useCallback, useState } from "react";
import type { ExportFormat } from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { exportPortfolio, pickExportFolder } from "../gateway";
import { exportDoneMessage, exportErrorToI18n } from "../shared/presenter";

/** #509 (the validated mock) — what the section shows under its action. */
export type ExportOutcome =
  | { kind: "idle" }
  | { kind: "working" }
  | { kind: "done"; message: I18nMessage }
  | { kind: "failed"; message: I18nMessage };

/** #509 (the validated mock), EXP-010 / EXP-016 — the format, the export and what it gave. */
export function useYourDataSection() {
  const [format, setFormatState] = useState<ExportFormat>("Csv");
  const [outcome, setOutcome] = useState<ExportOutcome>({ kind: "idle" });

  // A new format clears the line about the last export: it described another format.
  const setFormat = useCallback((next: ExportFormat) => {
    setFormatState(next);
    setOutcome({ kind: "idle" });
  }, []);

  const exportData = useCallback(async () => {
    try {
      const folder = await pickExportFolder();
      if (folder === null) return;
      setOutcome({ kind: "working" });
      const result = await exportPortfolio(format, folder);
      if (result.status === "ok") {
        logger.info("[YourDataSection] exported");
        setOutcome({ kind: "done", message: exportDoneMessage(result.data) });
      } else {
        logger.warn("[YourDataSection] export failed", { code: result.error.code });
        setOutcome({ kind: "failed", message: exportErrorToI18n(result.error) });
      }
    } catch (error) {
      logger.error("[YourDataSection] export threw", { error });
      setOutcome({ kind: "failed", message: { key: "settings.your_data.failed.write_failed" } });
    }
  }, [format]);

  return { format, setFormat, outcome, exportData };
}
