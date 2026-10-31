import { useTranslation } from "react-i18next";
import type { FileRead, ImportMapperDraft, MappedColumns, OperationType } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextField } from "@/ui/components/field/TextField";
import {
  COLUMN_FIELDS,
  columnAsInFile,
  DATE_FORMAT_LABELS,
  DECIMAL_MARK_LABELS,
  ENCODING_LABELS,
  type FormatChoices,
  type FormatValue,
  OPERATION_TYPE_LABELS,
  SEPARATOR_LABELS,
  typeOfLabel,
} from "./presenter";

interface MapperFormProps {
  /** Null until the file reads: only its format can be chosen then (IMP-061). */
  file: FileRead | null;
  draft: ImportMapperDraft;
  busy: boolean;
  saved: boolean;
  onFormat: (field: keyof FormatChoices, value: FormatValue) => void;
  onColumn: (field: keyof MappedColumns, value: string) => void;
  onLabelType: (label: string, type: OperationType | null) => void;
  onProvider: (provider: string) => void;
  onSave: () => void;
}

/** The mapper in use: how the file is read, its columns and its operations (IMP-018 to IMP-022). */
export function MapperForm({
  file,
  draft,
  busy,
  saved,
  onFormat,
  onColumn,
  onLabelType,
  onProvider,
  onSave,
}: MapperFormProps) {
  const { t } = useTranslation();
  const optionsOf = (labels: Record<string, string>) =>
    Object.entries(labels).map(([value, key]) => ({ value, label: t(key) }));
  const formats: { field: keyof FormatChoices; label: string; labels: Record<string, string> }[] = [
    { field: "encoding", label: "account_import.encoding_label", labels: ENCODING_LABELS },
    { field: "separator", label: "account_import.separator_label", labels: SEPARATOR_LABELS },
    { field: "decimal_mark", label: "account_import.decimal_label", labels: DECIMAL_MARK_LABELS },
    { field: "date_format", label: "account_import.date_format_label", labels: DATE_FORMAT_LABELS },
  ];
  const columns = file?.columns ?? [];
  const columnOptions = (held: string) =>
    [...columns, ...(held && !columns.includes(held) ? [held] : [])].map((column) => ({
      value: column,
      label: column,
    }));

  return (
    <div className="flex flex-col gap-4">
      <section className="flex flex-col gap-2">
        <h3 className="text-sm font-semibold text-m3-on-surface">
          {t("account_import.format_heading")}
        </h3>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          {formats.map(({ field, label, labels }) => (
            <SelectField
              key={field}
              id={`account-import-${field.replace("_", "-")}`}
              label={t(label)}
              value={draft[field]}
              onChange={(e) => onFormat(field, e.target.value as FormatValue)}
              options={optionsOf(labels)}
            />
          ))}
        </div>
        {file?.date_reading?.detected === false && (
          <p id="account-import-date-undetected" className="text-xs text-m3-on-surface-variant">
            {t("account_import.date_not_detected")}
          </p>
        )}
      </section>

      {file && (
        <>
          <section className="flex flex-col gap-2">
            <h3 className="text-sm font-semibold text-m3-on-surface">
              {t("account_import.columns_heading")}
            </h3>
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
              {COLUMN_FIELDS.map(({ field, label, required }) => {
                const held = columnAsInFile(columns, draft.columns[field]);
                return (
                  <SelectField
                    key={field}
                    id={`account-import-column-${field.replace("_", "-")}`}
                    label={required ? `${t(label)} *` : t(label)}
                    value={held}
                    onChange={(e) => onColumn(field, e.target.value)}
                    options={[
                      {
                        value: "",
                        label: t(required ? "account_import.choose" : "account_import.none"),
                      },
                      ...columnOptions(held),
                    ]}
                  />
                );
              })}
            </div>
          </section>

          {file.operation_labels.length > 0 && (
            <section className="flex flex-col gap-2">
              <h3 className="text-sm font-semibold text-m3-on-surface">
                {t("account_import.operations_heading")}
              </h3>
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                {file.operation_labels.map(({ label, rows }, index) => (
                  <div
                    key={label}
                    className="grid grid-cols-[1fr_13rem] items-center gap-2 [&_label]:sr-only"
                  >
                    <span className="truncate text-sm text-m3-on-surface" title={label}>
                      {label} <span className="text-m3-on-surface-variant">({rows})</span>
                    </span>
                    <SelectField
                      id={`account-import-label-${index}`}
                      label={label}
                      className="pr-9"
                      value={typeOfLabel(draft, label) ?? ""}
                      onChange={(e) =>
                        onLabelType(label, (e.target.value || null) as OperationType | null)
                      }
                      options={[
                        { value: "", label: t("account_import.choose") },
                        ...optionsOf(OPERATION_TYPE_LABELS),
                      ]}
                    />
                  </div>
                ))}
              </div>
              <p className="text-xs text-m3-on-surface-variant">
                {t("account_import.operations_hint")}
              </p>
            </section>
          )}

          <form
            id="account-import-mapper-form"
            className="grid grid-cols-[1fr_auto] items-end gap-3"
            onSubmit={(e) => {
              e.preventDefault();
              onSave();
            }}
          >
            <TextField
              id="account-import-provider"
              label={t("account_import.provider_label")}
              type="text"
              value={draft.info.provider}
              onChange={(e) => onProvider(e.target.value)}
              placeholder={t("account_import.provider_placeholder")}
            />
            <Button id="account-import-save" type="submit" variant="tonal" loading={busy}>
              {t("account_import.action_save")}
            </Button>
            {saved && (
              <p
                id="account-import-saved"
                role="status"
                className="text-xs text-m3-on-surface-variant"
              >
                {t("account_import.saved")}
              </p>
            )}
          </form>
        </>
      )}
    </div>
  );
}
