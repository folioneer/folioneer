import { useTranslation } from "react-i18next";
import type { PriceMovementReport } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import {
  formatPriceMovementAmount,
  formatPriceMovementDate,
  formatPriceMovementPct,
  formatPriceMovementValue,
} from "../shared/presenter";

interface PriceMovementDialogProps {
  report: PriceMovementReport;
  isOpen: boolean;
  onDismiss: () => void;
}

/**
 * PMV-028 — the project's financial polarity for a signed movement, whether
 * a percentage or an amount: gain and loss carry their own intent tokens rather
 * than overloading success/error (see `PnlCell`). A null movement is neither, so
 * it stays neutral.
 */
function movementColour(signedMovement: number | null): string {
  if (signedMovement === null || signedMovement === 0) return "text-m3-on-surface-variant";
  return signedMovement > 0 ? "text-m3-gain" : "text-m3-loss";
}

/**
 * PMV-013/017/027/028/030–034/043/046/050–052/060 — what the last Global refresh did to each
 * account's value, as a dialog the user reads once and closes.
 *
 * It waits for the unupdated-prices modal rather than stacking over it
 * (PMV-018); the caller owns that ordering.
 */
export function PriceMovementDialog({ report, isOpen, onDismiss }: PriceMovementDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog
      id="price-movement-dialog"
      isOpen={isOpen}
      onClose={onDismiss}
      title={t("pmv.title")}
      maxWidth="max-w-2xl"
      actions={
        <Button id="price-movement-dismiss" variant="tonal" onClick={onDismiss}>
          {t("pmv.dismiss")}
        </Button>
      }
    >
      <PriceMovementReportBody report={report} />
    </Dialog>
  );
}

/**
 * The report itself, free of the dialog chrome. Every judgement it displays was
 * made by the backend — whether a proportion exists, the row order, what counts
 * as incomplete — so this renders and never derives.
 *
 * Separate from the chrome so the report can be rendered and asserted without
 * the dialog's overlay machinery — which is also what lets the visual preview
 * put four states on one page, where four `fixed inset-0` dialogs could not
 * (`docs/frontend-visual-proof.md`).
 */
export function PriceMovementReportBody({ report }: { report: PriceMovementReport }) {
  const { t, i18n } = useTranslation();
  // PMV-060 — decided by the values alone, never by the dates.
  const nothingMoved = report.rows.every((row) => row.before === row.after);

  return (
    <>
      <p className="text-xs text-m3-on-surface-variant">{t("pmv.subtitle_frozen")}</p>

      {nothingMoved ? (
        <p className="mt-4 text-sm text-m3-on-surface">
          {report.incomplete ? t("pmv.nothing_moved_incomplete") : t("pmv.nothing_moved")}
        </p>
      ) : (
        <div className="mt-4 overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-m3-surface-variant text-xs text-m3-on-surface-variant">
                <th className="py-2 text-left font-medium">{t("pmv.column_account")}</th>
                <th className="py-2 text-right font-medium tabular-nums">
                  {t("pmv.column_before", {
                    date: formatPriceMovementDate(report.observed_from, i18n.language),
                  })}
                </th>
                <th className="py-2 pl-4 text-right font-medium tabular-nums">
                  {t("pmv.column_after", {
                    date: formatPriceMovementDate(report.observed_to, i18n.language),
                  })}
                </th>
                <th className="py-2 pl-4 text-right font-medium">{t("pmv.column_amount")}</th>
                <th className="py-2 pl-4 text-right font-medium">{t("pmv.column_change")}</th>
              </tr>
            </thead>
            <tbody>
              {report.rows.map((row) => (
                <tr
                  key={row.account_id}
                  id={`price-movement-row-${row.account_id}`}
                  className="border-b border-m3-surface-variant/40"
                >
                  <td className="py-2.5 text-left">
                    <span className="text-m3-on-surface">{row.name}</span>
                    {row.incomplete && (
                      <span className="ml-2 text-xs text-m3-on-surface-variant">
                        {t("pmv.incomplete_marker")}
                      </span>
                    )}
                  </td>
                  <td className="py-2.5 text-right tabular-nums text-m3-on-surface-variant">
                    {`${formatPriceMovementValue(row.before)} ${row.currency}`}
                  </td>
                  <td className="py-2.5 pl-4 text-right font-medium tabular-nums text-m3-on-surface">
                    {`${formatPriceMovementValue(row.after)} ${row.currency}`}
                  </td>
                  <td
                    className={`py-2.5 pl-4 text-right font-medium tabular-nums ${movementColour(row.movement_amount)}`}
                  >
                    {formatPriceMovementAmount(row.movement_amount, row.currency)}
                  </td>
                  <td
                    className={`py-2.5 pl-4 text-right font-medium tabular-nums ${movementColour(row.movement_pct)}`}
                  >
                    {formatPriceMovementPct(row.movement_pct)}
                  </td>
                </tr>
              ))}
              <tr id="price-movement-total" className="border-t border-m3-surface-variant">
                <td className="py-2.5 text-left font-semibold text-m3-on-surface">
                  {t("pmv.total")}
                  {report.incomplete && (
                    <span className="ml-2 text-xs font-normal text-m3-on-surface-variant">
                      {t("pmv.incomplete_marker")}
                    </span>
                  )}
                </td>
                <td className="py-2.5 text-right tabular-nums text-m3-on-surface-variant">
                  {`${formatPriceMovementValue(report.total_before)} ${report.total_currency}`}
                </td>
                <td className="py-2.5 pl-4 text-right font-semibold tabular-nums text-m3-on-surface">
                  {`${formatPriceMovementValue(report.total_after)} ${report.total_currency}`}
                </td>
                <td
                  className={`py-2.5 pl-4 text-right font-semibold tabular-nums ${movementColour(report.total_movement_amount)}`}
                >
                  {formatPriceMovementAmount(report.total_movement_amount, report.total_currency)}
                </td>
                <td
                  className={`py-2.5 pl-4 text-right font-semibold tabular-nums ${movementColour(report.total_movement_pct)}`}
                >
                  {formatPriceMovementPct(report.total_movement_pct)}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
