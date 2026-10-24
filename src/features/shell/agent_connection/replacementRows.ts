import type { TFunction } from "i18next";
import type { ReplacedShown, TransferShown } from "@/bindings";
import { microToFormatted, microToFormattedQuantity } from "@/lib/microUnits";
import { formatIsoDateNumeric } from "@/ui/format/date";
import type { TransactionRow } from "./transactionRows";

/** One cell of a replaced transaction's line: a stable key and its value. */
type ReplacedCell = [key: string, value: string];

const MINUS = "−";

/** A figure the core signed, shown with its sign: a plus in, a minus out. */
function signed(moved: number, format: (magnitude: number) => string): string {
  return `${moved < 0 ? MINUS : "+"}${format(Math.abs(moved))}`;
}

/**
 * AGT-092 — one of the four transactions a replacement removes, on one line: the account,
 * the date, the type, the asset — "Cash" for a transaction on the account's cash —, the
 * quantity — none on the cash; leaving the holding with a minus, entering it with a plus —
 * and the total with the account's currency, a plus into the account's cash and a minus out
 * of it.
 */
export function replacedRow(
  replaced: ReplacedShown,
  t: TFunction,
  language: string,
): ReplacedCell[] {
  const shown = replaced.transaction;
  const quantity =
    replaced.holding_moved === null ? "" : signed(replaced.holding_moved, microToFormattedQuantity);
  return [
    ["account", shown.account],
    ["date", formatIsoDateNumeric(shown.date, language)],
    ["kind", t(`transaction.type_${shown.kind.toLowerCase()}`)],
    ["asset", shown.asset_name ?? t("agent.replacement_cash")],
    ["quantity", quantity],
    [
      "amount",
      `${signed(replaced.cash_moved, (magnitude) => microToFormatted(magnitude, 2))} ${shown.currency}`,
    ],
  ];
}

/**
 * AGT-092 — the transfer recorded in the four's place: from, to, date, asset, the quantity
 * sent and received, and the cost moved — between currencies, at its rate into the receiving
 * account's. `feeInKind` is the quantity not received, formatted, when the core gives one.
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
    feeInKind:
      transfer.quantity_not_received === null
        ? null
        : microToFormattedQuantity(transfer.quantity_not_received),
  };
}
