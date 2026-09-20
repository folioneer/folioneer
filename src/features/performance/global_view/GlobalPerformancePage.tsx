import { Link } from "@tanstack/react-router";
import { ArrowLeft, Plus } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { AccountPerformanceTable } from "../account_view/AccountPerformanceTable";
import { AccountValueChart } from "../value_chart/AccountValueChart";
import { useGlobalPerformance } from "./useGlobalPerformance";

/**
 * GPF — portfolio-wide performance page. Mirrors AccountPerformancePage with two
 * scope selectors (account + asset, GPF-010); the chart and period table are the
 * account view's presentational components, fed by this view's hook.
 */
export function GlobalPerformancePage() {
  const { t } = useTranslation();
  const view = useGlobalPerformance();

  useEffect(() => {
    logger.info("[GlobalPerformancePage] mounted");
  }, []);

  const showYtdColumn = view.viewMode === "month";
  const showAnnualizedColumn = view.viewMode === "year";

  return (
    <div className="flex h-full flex-col gap-4 overflow-hidden py-2 px-2">
      <div className="flex-1 flex flex-col min-w-0 bg-m3-surface-container rounded-[28px] shadow-elevation-1 overflow-hidden">
        {/* Header */}
        <div className="px-6 py-4 bg-m3-surface-container-high flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            {/* Back navigation to the accounts overview */}
            <Link
              to="/accounts"
              aria-label={t("global_performance.back")}
              className="inline-flex items-center gap-1 text-sm text-m3-on-surface-variant hover:text-m3-on-surface"
            >
              <span
                id="global-performance-back"
                data-testid="global-performance-back"
                className="inline-flex items-center gap-1"
              >
                <ArrowLeft size={16} />
                {t("global_performance.back")}
              </span>
            </Link>
            <h2 className="text-base font-semibold text-m3-on-surface">
              {t("global_performance.title")}
              {/* GPF-011 — the scoped account/asset names appear in the title; absent for the whole portfolio. */}
              {view.scopeLabel !== null && (
                <span
                  data-testid="global-performance-scope-label"
                  className="font-normal text-m3-on-surface-variant"
                >
                  {" — "}
                  {view.scopeLabel}
                </span>
              )}
              {/* GPF-011 — every figure is reported in the response currency (EUR for cross-account scopes). */}
              {view.currency !== null && (
                <span
                  data-testid="global-performance-currency"
                  className="font-normal text-m3-on-surface-variant"
                >
                  {" · "}
                  {view.currency}
                </span>
              )}
            </h2>
          </div>

          {/* GPF-014 — view-mode toggle, present only when month view is available */}
          {view.monthViewAvailable && !view.isLoading && !view.error && !view.isEmpty && (
            <fieldset
              id="global-performance-view-toggle"
              data-testid="global-performance-view-toggle"
              className="flex gap-1 border-0 p-0 m-0"
              aria-label={t("account_performance.view_toggle_label")}
            >
              <Button
                id="global-performance-view-toggle-month"
                data-testid="global-performance-view-toggle-month"
                variant={view.viewMode === "month" ? "tonal" : "secondary"}
                size="sm"
                onClick={() => view.setViewMode("month")}
                aria-label={t("account_performance.view_month")}
              >
                {t("account_performance.view_month")}
              </Button>
              <Button
                id="global-performance-view-toggle-year"
                data-testid="global-performance-view-toggle-year"
                variant={view.viewMode === "year" ? "tonal" : "secondary"}
                size="sm"
                onClick={() => view.setViewMode("year")}
                aria-label={t("account_performance.view_year")}
              >
                {t("account_performance.view_year")}
              </Button>
            </fieldset>
          )}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-auto">
          {view.isLoading ? (
            <div data-testid="global-performance-loading" className="animate-pulse p-4 space-y-3">
              {[1, 2, 3].map((i) => (
                <div key={i} className="h-10 bg-m3-surface-variant rounded-lg" />
              ))}
            </div>
          ) : view.error ? (
            <div
              data-testid="global-performance-error"
              className="flex flex-col items-center justify-center h-full gap-3 py-12"
            >
              <span className="text-m3-error text-sm">{t(view.error.key, view.error.vars)}</span>
              <Button
                id="global-performance-retry"
                data-testid="global-performance-retry"
                variant="secondary"
                size="sm"
                onClick={view.retry}
              >
                {t("account_performance.retry")}
              </Button>
            </div>
          ) : view.isEmpty ? (
            /* GPF-015 — empty portfolio with Add Transaction affordance */
            <div
              id="global-performance-empty"
              data-testid="global-performance-empty"
              className="flex flex-col items-center justify-center h-full gap-4 py-12"
            >
              <p className="text-m3-on-surface-variant italic">{t("account_performance.empty")}</p>
              <Link
                to="/transactions/new"
                search={{
                  prefillAccountId: view.selectedAccountId ?? undefined,
                  prefillAssetId: undefined,
                }}
                aria-label={t("account_performance.add_transaction")}
              >
                <Button
                  id="global-performance-add-transaction"
                  data-testid="global-performance-add-transaction"
                  variant="primary"
                  size="sm"
                  icon={<Plus size={14} />}
                >
                  {t("account_performance.add_transaction")}
                </Button>
              </Link>
            </div>
          ) : (
            <div className="flex flex-col gap-3 p-2">
              <div className="flex items-center gap-3 px-4 pt-2">
                {/* Year selector, present only in month view */}
                {view.viewMode === "month" && (
                  <div>
                    <label htmlFor="global-performance-year-selector" className="sr-only">
                      {t("account_performance.year_selector_label")}
                    </label>
                    <select
                      id="global-performance-year-selector"
                      data-testid="global-performance-year-selector"
                      className="rounded-lg bg-m3-surface-container-high px-3 py-1.5 text-sm text-m3-on-surface"
                      value={view.selectedYear ?? ""}
                      aria-label={t("account_performance.year_selector_label")}
                      onChange={(event) => view.setSelectedYear(Number(event.target.value))}
                    >
                      {view.availableYears.map((year) => (
                        <option key={year} value={year}>
                          {year}
                        </option>
                      ))}
                    </select>
                  </div>
                )}

                {/* GPF-010 — account scope selector; "All accounts" = whole portfolio. */}
                <div>
                  <label htmlFor="global-performance-account-selector" className="sr-only">
                    {t("global_performance.account_selector_label")}
                  </label>
                  <select
                    id="global-performance-account-selector"
                    data-testid="global-performance-account-selector"
                    className="rounded-lg bg-m3-surface-container-high px-3 py-1.5 text-sm text-m3-on-surface"
                    value={view.selectedAccountId ?? ""}
                    aria-label={t("global_performance.account_selector_label")}
                    onChange={(event) =>
                      view.setSelectedAccountId(
                        event.target.value === "" ? null : event.target.value,
                      )
                    }
                  >
                    <option value="">{t("global_performance.account_selector_all")}</option>
                    {view.accountOptions.map((option) => (
                      <option key={option.accountId} value={option.accountId}>
                        {option.accountName}
                      </option>
                    ))}
                  </select>
                </div>

                {/* GPF-010 — asset scope selector; options follow the account scope. */}
                <div>
                  <label htmlFor="global-performance-asset-selector" className="sr-only">
                    {t("account_performance.asset_selector_label")}
                  </label>
                  <select
                    id="global-performance-asset-selector"
                    data-testid="global-performance-asset-selector"
                    className="rounded-lg bg-m3-surface-container-high px-3 py-1.5 text-sm text-m3-on-surface"
                    value={view.selectedAssetId ?? ""}
                    aria-label={t("account_performance.asset_selector_label")}
                    onChange={(event) =>
                      view.setSelectedAssetId(event.target.value === "" ? null : event.target.value)
                    }
                  >
                    <option value="">{t("account_performance.asset_selector_all")}</option>
                    {view.assetOptions.map((option) => (
                      <option key={option.assetId} value={option.assetId}>
                        {option.assetName}
                      </option>
                    ))}
                  </select>
                </div>
              </div>

              {/* Portfolio value over time — fed by the same active-view series as the table. */}
              <AccountValueChart points={view.chartPoints} idPrefix="global-performance" />

              <AccountPerformanceTable
                rows={view.rows}
                showYtd={showYtdColumn}
                showAnnualized={showAnnualizedColumn}
                idPrefix="global-performance"
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
