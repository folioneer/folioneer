import { useTranslation } from "react-i18next";
import { useAppStore } from "@/lib/store";

/**
 * MKT-180 — thin determinate progress bar shown in the shell (below the header,
 * visible on every page) while a market-price fetch task is running. Driven by
 * the store's `priceFetch` slice, fed by `AssetPriceFetchProgress` events and
 * cleared on `AssetPriceFetchCompleted`.
 */
export function PriceFetchProgressBar() {
  const { t } = useTranslation();
  const priceFetch = useAppStore((state) => state.priceFetch);

  if (!priceFetch.active) return null;

  const percent = priceFetch.total > 0 ? Math.round((priceFetch.done / priceFetch.total) * 100) : 0;

  // Overlaid on the content edge (absolute in a zero-height anchor) so showing or
  // hiding the bar never shifts the page layout — a mid-fetch reflow made CI
  // WebDriver clicks land on moved elements ("element click intercepted").
  return (
    <div className="relative z-20 h-0">
      <div
        id="price-fetch-progress"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
        aria-label={t("mkt.fetch_progress_label", {
          done: priceFetch.done,
          total: priceFetch.total,
        })}
        title={t("mkt.fetch_progress_label", {
          done: priceFetch.done,
          total: priceFetch.total,
        })}
        className="absolute inset-x-0 top-0 h-1 bg-m3-surface-variant"
      >
        <div
          className="h-full bg-m3-primary transition-[width] duration-300"
          style={{ width: `${percent}%` }}
        />
      </div>
    </div>
  );
}
