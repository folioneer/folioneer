import { ChartLine } from "lucide-react";
import { useTranslation } from "react-i18next";
import { selectHasExternalProvider, useAppStore } from "@/lib/store";
import { formatIsoDateNumeric, formatIsoDateTimeNumeric } from "@/ui/format/date";
import { usePriceFreshness } from "./usePriceFreshness";

/**
 * MKT-202 — compact shell item: "Prices as of" the newest price date among the held
 * assets; on hover, when this computer last fetched prices. Shown whether or not sync
 * is enabled, and absent until its figures have been read. In a build without an
 * External provider the date stays and the item says nothing about fetching (MKT-212).
 */
export function PriceFreshnessIndicator() {
  const { t, i18n } = useTranslation();
  const { visible, newestPriceDate, lastFetchAt } = usePriceFreshness();
  const hasExternalProvider = useAppStore(selectHasExternalProvider);

  if (!visible) {
    return null;
  }

  const asOf =
    newestPriceDate === null
      ? t("price_freshness.none")
      : t("price_freshness.as_of", { date: formatIsoDateNumeric(newestPriceDate, i18n.language) });
  const lastFetch = !hasExternalProvider
    ? undefined
    : lastFetchAt === null
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
