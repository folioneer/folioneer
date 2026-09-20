import { useTranslation } from "react-i18next";

type PnlCellProps = { value: string; raw: number };

export function PnlCell({ value, raw }: PnlCellProps) {
  const { t } = useTranslation();
  const colorClass =
    raw > 0 ? "text-m3-gain" : raw < 0 ? "text-m3-loss" : "text-m3-on-surface-variant";
  return (
    <span className={`tabular-nums ${colorClass}`}>
      {raw === 0 ? t("account_details.pnl_placeholder") : value}
    </span>
  );
}
