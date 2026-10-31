import { useTranslation } from "react-i18next";
import { microToFormattedQuantity } from "@/lib/microUnits";
import { formatIsoDateNumeric } from "@/ui/format/date";
import { messageText } from "@/ui/format/i18n";
import { moneyOf, type PreviewRowView } from "./previewPresenter";
import { StatusChip } from "./StatusChip";

interface ImportPreviewTableProps {
  rows: PreviewRowView[];
  currency: string;
  /** IMP-067 — an unknown asset's status opens its step; a row reading nothing does not. */
  onUnknown?: (line: number) => void;
}

/** The rows of the file as judged (IMP-066), in the file's order. */
export function ImportPreviewTable({ rows, currency, onUnknown }: ImportPreviewTableProps) {
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
              ? t("account_import.note_funding", {
                  amount: moneyOf(row.funding, currency, i18n.language),
                })
              : messageText(t, row.note);
          return (
            <tbody key={row.line} id={`account-import-row-${row.line}`}>
              <tr>
                <td className="m3-td">{row.line}</td>
                <td className="m3-td">
                  <StatusChip
                    id={
                      onUnknown && row.opensAsset ? `account-import-unknown-${row.line}` : undefined
                    }
                    tone={row.tone}
                    label={status}
                    onClick={onUnknown && row.opensAsset ? () => onUnknown(row.line) : undefined}
                  />
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
