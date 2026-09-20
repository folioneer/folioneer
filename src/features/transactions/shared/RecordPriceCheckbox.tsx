import { useTranslation } from "react-i18next";

interface RecordPriceCheckboxProps {
  /** Whether the auto-record toggle is on for this transaction. */
  checked: boolean;
  /** Toggle callback. */
  onChange: (checked: boolean) => void;
  /** ISO date used in the i18n label (MKT-051 — live updates with the form's date). */
  date: string;
}

/**
 * MKT-051 — Per-transaction "use this price as market price for {date}" checkbox.
 * Rendered immediately before the submit action in buy / sell / add / edit forms.
 */
export function RecordPriceCheckbox({ checked, onChange, date }: RecordPriceCheckboxProps) {
  const { t } = useTranslation();
  return (
    <label className="flex items-center gap-3 cursor-pointer group">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="accent-m3-primary w-4 h-4"
      />
      <span className="text-sm text-m3-on-surface group-hover:text-m3-primary transition-colors">
        {t("transaction.auto_record_price_label", { date })}
      </span>
    </label>
  );
}
