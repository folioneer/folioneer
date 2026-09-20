import { ChartLine } from "lucide-react";
import { useTranslation } from "react-i18next";
import { formatIsoDateNumeric, formatIsoDateTimeNumeric } from "@/ui/format/date";
import { usePriceFreshness } from "./usePriceFreshness";

/**
 * MKT-202 — compact shell item: "Prices as of" the newest price date among the held
 * assets; on hover, when this computer last fetched prices. Shown whether or not sync
 * is enabled, and absent until its figures have been read.
 */
export function PriceFreshnessIndicator() {
  const { t, i18n } = useTranslation();
  const { visible, newestPriceDate, lastFetchAt } = usePriceFreshness();

  if (!visible) {
    return null;
  }

  const asOf =
    newestPriceDate === null
      ? t("price_freshness.none")
      : t("price_freshness.as_of", { date: formatIsoDateNumeric(newestPriceDate, i18n.language) });
  const lastFetch =
    lastFetchAt === null
      ? t("price_freshness.never_fetched")
      : t("price_freshness.last_fetch", {
          when: formatIsoDateTimeNumeric(lastFetchAt, i18n.language),
        });

  return (
    <div
      id="price-freshness"
      data-testid="price-freshness"
      className="flex items-center gap-2 text-xs text-white/90"
      title={lastFetch}
    >
      <ChartLine size={14} aria-label={t("price_freshness.label")} role="img" />
      <span className="hidden sm:inline">{asOf}</span>
    </div>
  );
}
