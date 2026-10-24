import type { TFunction } from "i18next";
import type { TransactionShown, TransferShown } from "@/bindings";
import { microToFormatted, microToFormattedQuantity } from "@/lib/microUnits";
import { formatIsoDateNumeric } from "@/ui/format/date";
import type { TransactionRow } from "./transactionRows";

/** One cell of a replaced transaction's line: a stable key and its value. */
export type ReplacedCell = [key: string, value: string];

const MINUS = "−";

/** The kinds that bring money into the account's cash; the others take it out. */
const INTO_CASH = new Set(["Sell", "Deposit"]);
/** The kinds whose asset leaves the holding; a purchase enters it. */
const OUT_OF_HOLDING = new Set(["Sell"]);

/**
 * AGT-092 — one of the four transactions a replacement removes, on one line: the account,
 * the date, the type, the asset — "Cash" for a transaction on the account's cash —, the
 * quantity — none on the cash; leaving the holding with a minus, entering it with a plus —
 * and the total with the account's currency, a plus into the account's cash and a minus out
 * of it.
 */
export function replacedRow(
  shown: TransactionShown,
  t: TFunction,
  language: string,
): ReplacedCell[] {
  const onCash = shown.asset_name === null;
  const quantitySign = OUT_OF_HOLDING.has(shown.kind) ? MINUS : "+";
  const amountSign = INTO_CASH.has(shown.kind) ? "+" : MINUS;
  return [
    ["account", shown.account],
    ["date", formatIsoDateNumeric(shown.date, language)],
    ["kind", t(`transaction.type_${shown.kind.toLowerCase()}`)],
    ["asset", shown.asset_name ?? t("agent.replacement_cash")],
    ["quantity", onCash ? "" : `${quantitySign}${microToFormattedQuantity(shown.quantity)}`],
    ["amount", `${amountSign}${microToFormatted(Math.abs(shown.amount), 2)} ${shown.currency}`],
  ];
}

/**
 * AGT-092 — the transfer recorded in the four's place: from, to, date, asset, the quantity
 * sent and received, and the cost moved — between currencies, at its rate into the receiving
 * account's. `feeInKind` is the quantity not received, formatted, when less is received than
 * sent; null otherwise.
 */
export function transferRows(
  transfer: TransferShown,
  t: TFunction,
  language: string,
): { rows: TransactionRow[]; feeInKind: string | null } {
  const cost = `${microToFormatted(transfer.cost, 2)} ${transfer.from_currency}`;
  const costMoved =
    transfer.exchange_rate !== null && transfer.converted_cost !== null
      ? t("agent.replacement_cost_converted", {
          cost,
          converted: `${microToFormatted(transfer.converted_cost, 2)} ${transfer.to_currency}`,
          rate: microToFormattedQuantity(transfer.exchange_rate),
        })
      : cost;
  const asset = transfer.asset_reference
    ? `${transfer.asset_name} (${transfer.asset_reference})`
    : transfer.asset_name;
  const notReceived = transfer.quantity_sent - transfer.quantity_received;
  return {
    rows: [
      ["from", t("agent.replacement_from"), transfer.from_account],
      ["to", t("agent.replacement_to"), transfer.to_account],
      ["date", t("agent.transaction_date"), formatIsoDateNumeric(transfer.date, language)],
      ["asset", t("agent.transaction_asset"), asset],
      [
        "quantity",
        t("agent.transaction_quantity"),
        t("agent.replacement_quantity", {
          sent: microToFormattedQuantity(transfer.quantity_sent),
          received: microToFormattedQuantity(transfer.quantity_received),
        }),
      ],
      ["cost", t("agent.replacement_cost"), costMoved],
    ],
    feeInKind: notReceived > 0 ? microToFormattedQuantity(notReceived) : null,
  };
}
