import type { TFunction } from "i18next";
import type { ShownValue, TransactionShown } from "@/bindings";
import {
  decimalToMicro,
  formatQuantity,
  microToFormatted,
  microToFormattedFactor,
  microToFormattedPrice,
  microToFormattedQuantity,
} from "@/lib/microUnits";
import { formatIsoDateNumeric } from "@/ui/format/date";

/** One row of a transaction as the owner is shown it: a stable key, its label, its value. */
export type TransactionRow = [key: string, label: string, value: string];

/**
 * AGT-083 / AGT-089 — a transaction the owner typed, as its removal or change shows it: its
 * date, account, type, asset with its reference and quantity, and total with the account's
 * currency. A transaction on the account's cash has no asset, and no quantity of one; a
 * split's quantity is its factor.
 */
export function transactionRows(
  shown: TransactionShown,
  t: TFunction,
  language: string,
): TransactionRow[] {
  const rows: TransactionRow[] = [
    ["date", t("agent.transaction_date"), formatIsoDateNumeric(shown.date, language)],
    ["account", t("agent.transaction_account"), shown.account],
    ["kind", t("agent.transaction_kind"), t(`transaction.type_${shown.kind.toLowerCase()}`)],
  ];
  if (shown.asset_name !== null) {
    rows.push([
      "asset",
      t("agent.transaction_asset"),
      shown.asset_reference ? `${shown.asset_name} (${shown.asset_reference})` : shown.asset_name,
    ]);
    rows.push([
      "quantity",
      t("agent.transaction_quantity"),
      shown.kind === "Split"
        ? microToFormattedFactor(decimalToMicro(shown.quantity))
        : formatQuantity(shown.quantity),
    ]);
  }
  rows.push([
    "amount",
    t("agent.transaction_amount"),
    `${microToFormatted(shown.amount, 2)} ${shown.currency}`,
  ]);
  return rows;
}

/**
 * AGT-089 — the row of a value a change moves that the rows above do not show: the unit
 * price, the fees, the exchange rate or the note; none for the others.
 */
export function movedRow(
  value: ShownValue,
  shown: TransactionShown,
  t: TFunction,
): TransactionRow[] {
  switch (value) {
    case "unit_price":
      return [[value, t("agent.change_unit_price"), microToFormattedPrice(shown.unit_price)]];
    case "fees":
      return [[value, t("agent.change_fees"), microToFormatted(shown.fees, 2)]];
    case "exchange_rate":
      return [[value, t("agent.change_rate"), microToFormattedQuantity(shown.exchange_rate)]];
    case "note":
      return [[value, t("agent.change_note"), shown.note ?? ""]];
    default:
      return [];
  }
}
