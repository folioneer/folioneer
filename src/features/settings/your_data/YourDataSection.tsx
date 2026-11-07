import { useTranslation } from "react-i18next";
import type { ExportFormat } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { SelectField } from "@/ui/components/field/SelectField";
import { useYourDataSection } from "./useYourDataSection";

const FORMATS: { value: ExportFormat; labelKey: string }[] = [
  { value: "Csv", labelKey: "settings.your_data.format_csv" },
  { value: "Json", labelKey: "settings.your_data.format_json" },
];

/**
 * EXP-010 / EXP-016 — Settings' "Your data": the format, the Export… action and one line
 * saying where the export went, or that nothing was written. Stable ids per F25:
 * `your-data-format`, `your-data-export`, `your-data-outcome`.
 */
export function YourDataSection() {
  const { t } = useTranslation();
  const { format, setFormat, outcome, exportData } = useYourDataSection();
  const working = outcome.kind === "working";

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-m3-on-surface-variant">{t("settings.your_data.description")}</p>
      <SelectField
        id="your-data-format"
        label={t("settings.your_data.format_label")}
        value={format}
        onChange={(event) => setFormat(event.target.value as ExportFormat)}
        disabled={working}
        options={FORMATS.map(({ value, labelKey }) => ({ value, label: t(labelKey) }))}
      />
      <div>
        <Button
          id="your-data-export"
          variant="primary"
          onClick={() => void exportData()}
          disabled={working}
          loading={working}
        >
          {working ? t("settings.your_data.working") : t("settings.your_data.action_export")}
        </Button>
      </div>
      {(outcome.kind === "done" || outcome.kind === "failed") && (
        <p
          id="your-data-outcome"
          role="status"
          className={`text-sm ${outcome.kind === "failed" ? "text-m3-error" : "text-m3-on-surface"}`}
        >
          {t(outcome.message.key, outcome.message.vars)}
        </p>
      )}
    </div>
  );
}
