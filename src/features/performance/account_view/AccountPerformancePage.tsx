import { Link, useParams } from "@tanstack/react-router";
import { ArrowLeft, Plus } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { AccountValueChart } from "../value_chart/AccountValueChart";
import { AccountPerformanceTable } from "./AccountPerformanceTable";
import { useAccountPerformance } from "./useAccountPerformance";

export function AccountPerformancePage() {
  const { t } = useTranslation();
  const { accountId } = useParams({ from: "/accounts/$accountId/performance" });
  const view = useAccountPerformance(accountId);

  useEffect(() => {
    logger.info("[AccountPerformancePage] mounted");
  }, []);

  const showYtdColumn = view.viewMode === "month";
  const showAnnualizedColumn = view.viewMode === "year";

  return (
    <div className="flex h-full flex-col gap-4 overflow-hidden py-2 px-2">
      <div className="flex-1 flex flex-col min-w-0 bg-m3-surface-container rounded-[28px] shadow-elevation-1 overflow-hidden">
        {/* Header */}
        <div className="px-6 py-4 bg-m3-surface-container-high flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            {/* PRF-053 — back navigation to Account Details */}
            <Link
              to="/accounts/$accountId"
              params={{ accountId }}
              aria-label={t("account_performance.back")}
              className="inline-flex items-center gap-1 text-sm text-m3-on-surface-variant hover:text-m3-on-surface"
            >
              <span
                id="account-performance-back"
                data-testid="account-performance-back"
                className="inline-flex items-center gap-1"
              >
                <ArrowLeft size={16} />
                {t("account_performance.back")}
              </span>
            </Link>
            <h2 className="text-base font-semibold text-m3-on-surface">
              {t("account_performance.title")}
              {/* PRF-080 — the scoped asset's name appears in the title; absent for the whole account. */}
              {view.selectedAssetName !== null && (
                <span
                  data-testid="account-performance-scoped-asset-name"
                  className="font-normal text-m3-on-surface-variant"
                >
                  {" — "}
                  {view.selectedAssetName}
                </span>
              )}
            </h2>
          </div>

          {/* PRF-011 / PRF-013 — view-mode toggle, present only when month view is available */}
          {view.monthViewAvailable && !view.isLoading && !view.error && !view.isEmpty && (
            <fieldset
              id="account-performance-view-toggle"
              data-testid="account-performance-view-toggle"
              className="flex gap-1 border-0 p-0 m-0"
              aria-label={t("account_performance.view_toggle_label")}
            >
              <Button
                id="account-performance-view-toggle-month"
                data-testid="account-performance-view-toggle-month"
                variant={view.viewMode === "month" ? "tonal" : "secondary"}
                size="sm"
                onClick={() => view.setViewMode("month")}
                aria-label={t("account_performance.view_month")}
              >
                {t("account_performance.view_month")}
              </Button>
              <Button
                id="account-performance-view-toggle-year"
                data-testid="account-performance-view-toggle-year"
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
            /* PRF-050 — loading skeleton */
            <div data-testid="account-performance-loading" className="animate-pulse p-4 space-y-3">
              {[1, 2, 3].map((i) => (
                <div key={i} className="h-10 bg-m3-surface-variant rounded-lg" />
              ))}
            </div>
          ) : view.error ? (
            /* PRF-052 — error state with Retry */
            <div
              data-testid="account-performance-error"
              className="flex flex-col items-center justify-center h-full gap-3 py-12"
            >
              <span className="text-m3-error text-sm">{t(view.error.key, view.error.vars)}</span>
              <Button
                id="account-performance-retry"
                data-testid="account-performance-retry"
                variant="secondary"
                size="sm"
                onClick={view.retry}
              >
                {t("account_performance.retry")}
              </Button>
            </div>
          ) : view.isEmpty ? (
            /* PRF-051 — empty state with Add Transaction affordance */
            <div
              id="account-performance-empty"
              data-testid="account-performance-empty"
              className="flex flex-col items-center justify-center h-full gap-4 py-12"
            >
              <p className="text-m3-on-surface-variant italic">{t("account_performance.empty")}</p>
              <Link
                to="/transactions/new"
                search={{ prefillAccountId: accountId, prefillAssetId: undefined }}
                aria-label={t("account_performance.add_transaction")}
              >
                <Button
                  id="account-performance-add-transaction"
                  data-testid="account-performance-add-transaction"
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
                {/* PRF-015 — year selector, present only in month view */}
                {view.viewMode === "month" && (
                  <div>
                    <label htmlFor="account-performance-year-selector" className="sr-only">
                      {t("account_performance.year_selector_label")}
                    </label>
                    <select
                      id="account-performance-year-selector"
                      data-testid="account-performance-year-selector"
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

                {/* PRF-080 / PRF-082 — asset scope selector; "All assets" = whole account. */}
                <div>
                  <label htmlFor="account-performance-asset-selector" className="sr-only">
                    {t("account_performance.asset_selector_label")}
                  </label>
                  <select
                    id="account-performance-asset-selector"
                    data-testid="account-performance-asset-selector"
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

              {/* Account value over time — fed by the same active-view series as the table. */}
              <AccountValueChart points={view.chartPoints} />

              <AccountPerformanceTable
                rows={view.rows}
                showYtd={showYtdColumn}
                showAnnualized={showAnnualizedColumn}
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
