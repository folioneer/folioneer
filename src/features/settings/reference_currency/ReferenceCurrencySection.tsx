import { RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { SelectField } from "@/ui/components/field/SelectField";
import { currencyChoiceOptions } from "../shared/presenter";
import { useReferenceCurrency } from "./useReferenceCurrency";

/**
 * GPF-011 / FXR-129–131 — Settings section: the reference currency every total is reported
 * in, its help line and, while the rate histories are fetched, a line saying so. Stable ids
 * per F25: `settings-reference-currency`, `settings-reference-currency-fetching`,
 * `settings-reference-currency-error`.
 */
export function ReferenceCurrencySection() {
  const { t, i18n } = useTranslation();
  const { currency, choices, isFetchingHistory, isBusy, error, choose } = useReferenceCurrency();

  return (
    <section className="flex flex-col gap-2 rounded-xl border border-m3-outline/30 p-3">
      {currency !== null && (
        <SelectField
          id="settings-reference-currency"
          label={t("settings.reference_currency_label")}
          value={currency}
          disabled={isBusy}
          onChange={(event) => void choose(event.target.value)}
          options={currencyChoiceOptions(choices, i18n.language)}
        />
      )}
      <span className="text-xs text-m3-on-surface-variant">
        {t("settings.reference_currency_help")}
      </span>
      {isFetchingHistory && (
        <span
          id="settings-reference-currency-fetching"
          className="flex items-center gap-2 text-xs text-m3-primary"
        >
          <RefreshCw size={12} className="animate-spin" aria-hidden="true" />
          {t("settings.reference_currency_fetching")}
        </span>
      )}
      {error && (
        <span id="settings-reference-currency-error" role="alert" className="text-xs text-m3-error">
          {t(error.key, error.vars)}
        </span>
      )}
    </section>
  );
}
