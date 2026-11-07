import { Layers } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/ui/components/modal/Dialog";
import type { HoldingLotsTarget } from "../shared/types";
import { useHoldingLots } from "./useHoldingLots";

interface HoldingLotsModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  target: HoldingLotsTarget;
}

/**
 * #515 (the validated mock), CBM-034 — a holding's lots: acquired on, quantity, cost and
 * remaining, oldest first; a lot taken whole dimmed, free shares named.
 */
export function HoldingLotsModal({ isOpen, onClose, accountId, target }: HoldingLotsModalProps) {
  const { t } = useTranslation();
  const { rows, error } = useHoldingLots(accountId, target.assetId);

  return (
    <Dialog
      id="holding-lots-dialog"
      isOpen={isOpen}
      onClose={onClose}
      maxWidth="max-w-2xl"
      title={
        <span className="flex items-center gap-2">
          <Layers size={18} />
          {t("lots.title", { asset: target.assetName })}
        </span>
      }
    >
      <p id="holding-lots-summary" className="text-sm text-m3-on-surface-variant mb-3">
        {t("lots.summary", {
          held: target.quantity,
          cost: target.costBasis,
          currency: target.currency,
        })}
      </p>
      {error && (
        <p role="alert" className="text-sm text-m3-error">
          {t(error.key, error.vars)}
        </p>
      )}
      {rows === null && !error && (
        <p id="holding-lots-loading" className="text-sm text-m3-on-surface-variant">
          {t("lots.loading")}
        </p>
      )}
      {rows !== null && rows.length === 0 && (
        <p id="holding-lots-empty" className="text-sm text-m3-on-surface-variant">
          {t("lots.empty")}
        </p>
      )}
      {rows !== null && rows.length > 0 && (
        <table id="holding-lots-table" className="w-full border-collapse">
          <thead>
            <tr>
              <th className="m3-th text-left">{t("lots.column_acquired_on")}</th>
              <th className="m3-th text-right">{t("lots.column_quantity")}</th>
              <th className="m3-th text-right">
                {t("lots.column_cost", { currency: target.currency })}
              </th>
              <th className="m3-th text-right">{t("lots.column_remaining")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr
                key={row.key}
                id={`holding-lots-row-${row.key}`}
                className={`m3-tr ${row.spent ? "text-m3-on-surface-variant opacity-60" : ""}`}
              >
                <td className="m3-td">
                  {row.acquiredOn}
                  {row.free && (
                    <span className="ml-2 text-xs text-m3-on-surface-variant">
                      {t("lots.free_shares")}
                    </span>
                  )}
                </td>
                <td className="m3-td text-right tabular-nums">{row.quantity}</td>
                <td className="m3-td text-right tabular-nums">{row.cost}</td>
                <td className="m3-td text-right tabular-nums">{row.remaining}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Dialog>
  );
}
