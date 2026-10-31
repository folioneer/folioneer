import { useTranslation } from "react-i18next";
import { microToFormattedQuantity } from "@/lib/microUnits";
import { formatIsoDateNumeric } from "@/ui/format/date";
import { messageText } from "@/ui/format/i18n";
import { moneyOf, type PreviewRowView, type RowTone } from "./previewPresenter";

const TONES: Record<RowTone, string> = {
  ready: "bg-m3-primary text-m3-on-primary",
  neutral: "bg-m3-surface-container-highest text-m3-on-surface-variant",
  unknown: "bg-m3-warning-container text-m3-on-warning-container",
  rejected: "bg-m3-error-container text-m3-on-error-container",
};

const CHIP = "inline-flex whitespace-nowrap rounded-full px-2.5 py-1 text-xs font-medium";

/** A count or a status, in its status's colours (IMP-066). */
export function StatusChip({ tone, label, id }: { tone: RowTone; label: string; id?: string }) {
  return (
    <span id={id} className={`${CHIP} ${TONES[tone]}`}>
      {label}
    </span>
  );
}

interface ImportPreviewTableProps {
  rows: PreviewRowView[];
  currency: string;
  /** IMP-067 — an unknown asset's status opens its step; a row reading nothing does not. */
  onUnknown?: (line: number) => void;
  fundingNote: (amount: string) => string;
}

/** The rows of the file as judged (IMP-066), in the file's order. */
export function ImportPreviewTable({
  rows,
  currency,
  onUnknown,
  fundingNote,
}: ImportPreviewTableProps) {
  const { t, i18n } = useTranslation();
  const dash = "—";
  return (
    <div className="overflow-x-auto rounded-xl border border-m3-outline-variant">
      <table id="account-import-rows" className="m3-table w-full text-sm">
        <thead>
          <tr>
            <th className="m3-th">{t("account_import.column_line")}</th>
            <th className="m3-th">{t("account_import.column_status")}</th>
            <th className="m3-th">{t("account_import.column_row_date")}</th>
            <th className="m3-th">{t("account_import.column_row_operation")}</th>
            <th className="m3-th">{t("account_import.column_row_asset")}</th>
            <th className="m3-th text-right">{t("account_import.column_row_quantity")}</th>
            <th className="m3-th text-right">{t("account_import.column_row_amount")}</th>
          </tr>
        </thead>
        {rows.map((row) => {
          const status = messageText(t, row.status) ?? "";
          const note =
            row.funding !== null
              ? fundingNote(moneyOf(row.funding, currency, i18n.language))
              : messageText(t, row.note);
          return (
            <tbody key={row.line} id={`account-import-row-${row.line}`}>
              <tr>
                <td className="m3-td">{row.line}</td>
                <td className="m3-td">
                  {onUnknown && row.opensAsset ? (
                    <button
                      id={`account-import-unknown-${row.line}`}
                      type="button"
                      className={`${CHIP} ${TONES[row.tone]} cursor-pointer underline-offset-2 hover:underline`}
                      onClick={() => onUnknown(row.line)}
                    >
                      {status}
                    </button>
                  ) : (
                    <StatusChip tone={row.tone} label={status} />
                  )}
                </td>
                <td className="m3-td whitespace-nowrap">
                  {row.date ? formatIsoDateNumeric(row.date, i18n.language) : ""}
                </td>
                <td className="m3-td">{messageText(t, row.operation)}</td>
                <td className="m3-td">{row.assetName ?? (row.date ? dash : "")}</td>
                <td className="m3-td text-right">
                  {row.quantity !== null ? microToFormattedQuantity(row.quantity) : ""}
                </td>
                <td className="m3-td whitespace-nowrap text-right">
                  {row.amount !== null ? moneyOf(row.amount, currency, i18n.language) : ""}
                </td>
              </tr>
              {note && (
                <tr>
                  <td />
                  <td
                    id={`account-import-note-${row.line}`}
                    colSpan={6}
                    className="px-4 pb-2 text-xs text-m3-on-surface-variant"
                  >
                    {note}
                  </td>
                </tr>
              )}
            </tbody>
          );
        })}
      </table>
    </div>
  );
}
