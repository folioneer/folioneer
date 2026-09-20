import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { CurrencyRate } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { formatIsoDateNumeric } from "@/ui/format/date";
import { DeclarePairModal } from "../declare_pair/DeclarePairModal";
import { DeleteRateConfirmation, RecordRateModal } from "../record_rate/RecordRateModal";
import { formatRateMicros, formatRateSource } from "../shared/presenter";
import { useCurrencyRatesView } from "./useCurrencyRatesView";

/** FXR-050/051 — Currency Rates view: pair list with drill-in to rate history. */
export function CurrencyRatesView() {
  const { t, i18n } = useTranslation();
  const {
    isLoading,
    error,
    pairs,
    selectedPair,
    rates,
    ratesError,
    selectPair,
    clearSelection,
    refetch,
    isBackfilling,
    backfillHistory,
  } = useCurrencyRatesView();
  const showSnackbar = useSnackbar();

  // FXR-110 — one-click historical download; outcome lands in a snackbar.
  const handleBackfill = async () => {
    const outcome = await backfillHistory();
    if (outcome.status === "ok") {
      showSnackbar(t("currency.backfill_success", { count: outcome.ratesWritten }), "success");
    } else {
      // FXR-114 — the failure is surfaced distinctly, not as a neutral toast.
      showSnackbar(t(outcome.message.key, outcome.message.vars), "error");
    }
  };

  const [isAddPairOpen, setIsAddPairOpen] = useState(false);
  const [recordTarget, setRecordTarget] = useState<{ from: string; to: string } | null>(null);
  const [editTarget, setEditTarget] = useState<CurrencyRate | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<CurrencyRate | null>(null);

  let body: React.ReactNode;
  if (isLoading) {
    body = <div data-testid="currency-rates-loading">{t("currency.loading")}</div>;
  } else if (error) {
    body = (
      <p role="alert" data-testid="currency-rates-error" className="text-m3-error">
        {t(error.key, error.vars)}
      </p>
    );
  } else if (pairs.length === 0) {
    body = (
      <div data-testid="currency-rates-empty" className="text-m3-on-surface-variant">
        {t("currency.empty")}
      </div>
    );
  } else {
    body = (
      <table className="w-full">
        <tbody>
          {pairs.map((pair) => {
            const key = `${pair.from_currency}-${pair.to_currency}`;
            return (
              <tr
                key={key}
                className="m3-tr cursor-pointer"
                id={`pair-row-${key}`}
                data-testid={`pair-row-${key}`}
                tabIndex={0}
                aria-label={t("currency.open_pair", {
                  from: pair.from_currency,
                  to: pair.to_currency,
                })}
                onClick={() => selectPair(pair.from_currency, pair.to_currency)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    selectPair(pair.from_currency, pair.to_currency);
                  }
                }}
              >
                <td className="m3-td font-medium">{pair.from_currency}</td>
                <td className="m3-td font-medium">{pair.to_currency}</td>
                <td className="m3-td text-right tabular-nums">
                  {pair.latest_rate !== null ? (
                    <span>{formatRateMicros(pair.latest_rate)}</span>
                  ) : (
                    <span
                      data-testid={`pair-no-rate-${key}`}
                      className="text-m3-on-surface-variant"
                    >
                      —
                    </span>
                  )}
                </td>
                <td className="m3-td text-right">
                  {pair.latest_rate_source && (
                    <span className="text-xs text-m3-on-surface-variant">
                      {t(formatRateSource(pair.latest_rate_source) ?? "")}
                    </span>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    );
  }

  return (
    <>
      <div className="flex h-full flex-col gap-2 p-4">
        <div className="flex items-center justify-between">
          <h2 className="text-xl font-medium text-m3-on-surface">{t("currency.view_title")}</h2>
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              id="action-backfill-history"
              data-testid="action-backfill-history"
              loading={isBackfilling}
              disabled={isBackfilling}
              onClick={() => void handleBackfill()}
            >
              {t(isBackfilling ? "currency.backfill_running" : "currency.action_backfill_history")}
            </Button>
            <Button
              variant="tonal"
              size="sm"
              id="action-add-pair"
              data-testid="action-add-pair"
              onClick={() => setIsAddPairOpen(true)}
            >
              {t("currency.action_add_pair")}
            </Button>
          </div>
        </div>
        {/* When a pair is drilled into, the pairs list shrinks so the rate
            history below gets a bounded, scrollable region of its own — the
            shell's <main> is overflow-hidden, so anything outside this h-full
            column is unreachable (bit the EUR-USD history). */}
        <div
          className={selectedPair ? "max-h-[40%] shrink-0 overflow-auto" : "flex-1 overflow-auto"}
        >
          {body}
        </div>

        {selectedPair && (
          <div className="flex min-h-0 flex-1 flex-col gap-2 border-t border-neutral-30 pt-2">
            <div className="flex items-center justify-between">
              <h3 className="text-lg font-medium">
                {selectedPair.fromCurrency} → {selectedPair.toCurrency}
              </h3>
              <div className="flex items-center gap-2">
                <Button
                  variant="tonal"
                  size="sm"
                  id="currency-rates-action-record-rate"
                  data-testid="action-record-rate"
                  onClick={() =>
                    setRecordTarget({
                      from: selectedPair.fromCurrency,
                      to: selectedPair.toCurrency,
                    })
                  }
                >
                  {t("currency.action_record_rate")}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  id="currency-rates-close-drilldown"
                  onClick={clearSelection}
                >
                  {t("action.close")}
                </Button>
              </div>
            </div>
            {ratesError && (
              <p role="alert" data-testid="currency-rates-rates-error" className="text-m3-error">
                {t(ratesError.key, ratesError.vars)}
              </p>
            )}
            <div className="min-h-0 flex-1 overflow-y-auto">
              <table className="w-full">
                <tbody>
                  {rates.map((rate) => {
                    const rateKey = `${rate.from_currency}-${rate.to_currency}-${rate.date}`;
                    return (
                      <tr
                        key={rateKey}
                        id={`rate-row-${rateKey}`}
                        className="m3-tr"
                        data-testid={`rate-row-${rateKey}`}
                      >
                        <td className="m3-td tabular-nums">
                          {formatIsoDateNumeric(rate.date, i18n.language)}
                        </td>
                        <td className="m3-td text-right tabular-nums">
                          {formatRateMicros(rate.rate)}
                        </td>
                        <td className="m3-td text-right text-xs text-m3-on-surface-variant">
                          {t(formatRateSource(rate.source) ?? "")}
                        </td>
                        <td className="m3-td text-right">
                          <div className="flex items-center justify-end gap-2">
                            <Button
                              variant="ghost"
                              size="sm"
                              id={`action-edit-rate-${rateKey}`}
                              onClick={() => setEditTarget(rate)}
                            >
                              {t("action.edit")}
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              id={`action-delete-rate-${rateKey}`}
                              onClick={() => setDeleteTarget(rate)}
                            >
                              {t("action.delete")}
                            </Button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>

      <DeclarePairModal
        isOpen={isAddPairOpen}
        onClose={() => setIsAddPairOpen(false)}
        onSuccess={() => {
          setIsAddPairOpen(false);
          refetch();
        }}
      />

      {recordTarget && (
        <RecordRateModal
          isOpen
          fromCurrency={recordTarget.from}
          toCurrency={recordTarget.to}
          onClose={() => setRecordTarget(null)}
          onSuccess={() => setRecordTarget(null)}
        />
      )}

      {editTarget && (
        <RecordRateModal
          isOpen
          fromCurrency={editTarget.from_currency}
          toCurrency={editTarget.to_currency}
          initialRate={editTarget}
          onClose={() => setEditTarget(null)}
          onSuccess={() => setEditTarget(null)}
        />
      )}

      {deleteTarget && (
        <DeleteRateConfirmation
          isOpen
          rate={deleteTarget}
          onClose={() => setDeleteTarget(null)}
          onSuccess={() => setDeleteTarget(null)}
        />
      )}
    </>
  );
}
