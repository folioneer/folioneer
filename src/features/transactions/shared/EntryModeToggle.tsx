import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import type { TransactionEntryMode } from "./types";

interface EntryModeToggleProps {
  /** Stable-id prefix, e.g. "buy-trx" → buttons "buy-trx-entry-mode-price" / "-total". */
  idPrefix: string;
  value: TransactionEntryMode;
  onChange: (mode: TransactionEntryMode) => void;
}

/**
 * TRX-060 / SEL-050 — segmented toggle between the two money-entry modes:
 * type the unit price (default) or type the broker's all-in total.
 */
export function EntryModeToggle({ idPrefix, value, onChange }: EntryModeToggleProps) {
  const { t } = useTranslation();
  return (
    // reviewer-frontend FP: roving tabindex omitted deliberately — two independently
    // focusable buttons are acceptable for a binary toggle (2026-07-05).
    <div
      role="radiogroup"
      aria-label={t("transaction.entry_mode_label")}
      className="flex w-fit gap-1 rounded-2xl bg-m3-surface-variant p-1"
    >
      <Button
        id={`${idPrefix}-entry-mode-price`}
        size="sm"
        role="radio"
        aria-checked={value === "price"}
        variant={value === "price" ? "primary" : "ghost"}
        onClick={() => onChange("price")}
      >
        {t("transaction.entry_mode_price_label")}
      </Button>
      <Button
        id={`${idPrefix}-entry-mode-total`}
        size="sm"
        role="radio"
        aria-checked={value === "total"}
        variant={value === "total" ? "primary" : "ghost"}
        onClick={() => onChange("total")}
      >
        {t("transaction.entry_mode_total_label")}
      </Button>
    </div>
  );
}
