import type { AccountError, AssetLookupResult, AssetRead, PreviewRow, RowReason } from "@/bindings";
import { formatQuantity } from "@/lib/microUnits";
import type { I18nMessage } from "@/ui/format/i18n";
import { OPERATION_TYPE_LABELS } from "./presenter";

export type RowTone = "ready" | "neutral" | "unknown" | "rejected";

/** A row of the preview as the table shows it (IMP-066): figures still in micro-units. */
export interface PreviewRowView {
  line: number;
  status: I18nMessage;
  tone: RowTone;
  date: string | null;
  /** The operation's label key, or the file's own label when it is not imported. */
  operation: I18nMessage | null;
  assetName: string | null;
  quantity: number | null;
  amount: number | null;
  /** The deposit a purchase needs (IMP-038). */
  funding: number | null;
  note: I18nMessage | null;
  /** IMP-067 — the row's status opens its unknown asset's step; not when it reads nothing. */
  opensAsset: boolean;
}

/** An amount in the account's currency, as the mock writes it. */
export function moneyOf(micros: number, currency: string, locale: string): string {
  const amount = micros / 1_000_000;
  return currency
    ? new Intl.NumberFormat(locale, { style: "currency", currency }).format(amount)
    : new Intl.NumberFormat(locale, { minimumFractionDigits: 2 }).format(amount);
}

const assetNameOf = (asset: AssetRead) => asset.name ?? asset.reference ?? asset.isin;

function refusalToI18n(refusal: AccountError): I18nMessage {
  switch (refusal.code) {
    case "Oversell":
      return {
        key: "account_import.reason_over_held",
        vars: { held: formatQuantity(refusal.available) },
      };
    case "ClosedPosition":
      return { key: "account_import.reason_none_held" };
    case "CascadingOversell":
      return { key: "transaction.error_later_left_short" };
    case "InsufficientCash":
      return { key: "account_import.reason_no_cash" };
    default:
      return { key: `error.${refusal.code}` };
  }
}

/** F27 — why a row is rejected (IMP-030, IMP-035, IMP-037). */
export function rowReasonToI18n(reason: RowReason): I18nMessage {
  switch (reason.code) {
    case "OtherCurrency":
      return { key: "account_import.reason_other_currency" };
    case "AssetNotHeld":
      return { key: "account_import.reason_not_held" };
    case "ManualEntry":
      return refusalToI18n(reason.refusal);
    case "Unread": {
      const { rejection } = reason;
      switch (rejection.code) {
        case "LineTooShort":
          return { key: "account_import.reason_line_too_short" };
        case "OperationTypeMissing":
          return { key: "account_import.reason_no_type", vars: { label: rejection.label } };
        case "DateUnreadable":
          return { key: "account_import.reason_date", vars: { column: rejection.column } };
        case "FigureUnreadable":
          return { key: "account_import.reason_figure", vars: { column: rejection.column } };
        case "FigureMissing":
          return {
            key: "account_import.reason_figure_missing",
            vars: { field: rejection.field },
          };
        case "QuantityZero":
          return { key: "account_import.reason_quantity_zero" };
        case "TradeTaxUnpaired":
          return { key: "account_import.reason_tax_unpaired" };
      }
    }
  }
}

const empty = {
  date: null,
  operation: null,
  assetName: null,
  quantity: null,
  funding: null,
  note: null,
  opensAsset: false,
};

function ofOperation(operation: Extract<PreviewRow["status"], { status: "Ready" }>["operation"]) {
  const { figures } = operation;
  return {
    date: operation.date,
    operation: { key: OPERATION_TYPE_LABELS[figures.type] },
    assetName: assetNameOf(operation.asset),
    quantity: "quantity" in figures ? (figures.quantity ?? null) : null,
  };
}

/** The table's view of a row (IMP-066). */
export function rowViewOf(row: PreviewRow): PreviewRowView {
  const base = { ...empty, line: row.line, amount: row.amount };
  const { status } = row;
  switch (status.status) {
    case "Ready":
      return {
        ...base,
        ...ofOperation(status.operation),
        status: { key: "account_import.status_ready" },
        tone: "ready",
        funding: status.funding,
      };
    case "AlreadyRecorded":
      return {
        ...base,
        ...ofOperation(status.operation),
        status: { key: "account_import.status_already_recorded" },
        tone: "neutral",
      };
    case "UnknownAsset":
      return {
        ...base,
        ...ofOperation(status.operation),
        status: { key: "account_import.status_unknown_asset" },
        tone: "unknown",
        opensAsset:
          status.asset.isin !== null ||
          status.asset.reference !== null ||
          status.asset.name !== null,
      };
    case "JoinedTradeTax":
      return {
        ...base,
        ...ofOperation(status.operation),
        status: { key: "account_import.status_joined", vars: { line: status.purchase_line } },
        tone: "neutral",
      };
    case "Rejected":
      return {
        ...base,
        ...(status.operation ? ofOperation(status.operation) : {}),
        status: { key: "account_import.status_rejected" },
        tone: "rejected",
        note: rowReasonToI18n(status.reason),
      };
    case "NotImported":
      return {
        ...base,
        date: status.date,
        operation: { key: "account_import.operation_as_read", vars: { label: status.label } },
        assetName: assetNameOf(status.asset),
        status: { key: "account_import.status_not_imported" },
        tone: "neutral",
        note: { key: "account_import.note_not_imported" },
      };
  }
}

/** IMP-052 — the usual asset form, prefilled with what the row read and the account's currency. */
export function assetPrefillOf(asset: AssetRead, currency: string): AssetLookupResult {
  return {
    name: asset.name ?? "",
    reference: asset.reference,
    isin: asset.isin,
    currency,
    asset_class: null,
    exchange: null,
  };
}
