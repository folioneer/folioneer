import type {
  AccountDetailsResponse,
  AccountError,
  AssetError,
  AssetPriceSource,
  ClosedHoldingDetail,
  DividendError,
  FreeSharesError,
  HoldingDetail,
  HoldingInconsistency,
  HoldingPeriodPerformance,
  InterestError,
  ManagementFeeError,
  PriceHistoryBackfillError,
  PriceHistoryBackfillOutcome,
  SplitError,
} from "@/bindings";
import { isCashAsset } from "@/lib/cashAsset";
import {
  microToFormatted,
  microToFormattedPrice,
  microToFormattedQuantity,
} from "@/lib/microUnits";
import type { StoredPerfPeriod } from "@/lib/perfPeriodStorage";
import { formatHoldingInconsistency } from "@/ui/format/holdingInconsistency";
import type { I18nMessage } from "@/ui/format/i18n";
import { formatStalenessLabel, type StalenessLabel } from "@/ui/format/staleness";
import type { PriceableAsset } from "./types";

/**
 * F27 — Maps any asset-price mutation error (record / update / delete /
 * get prices) to an i18n key + optional interpolation vars. Pure function:
 * no React, no useTranslation. `err` is the BC-wide `AssetError` union, so the
 * switch lists the codes these price commands can raise and falls back to a
 * generic key for any other.
 */
export function assetPriceMutationErrorToI18n(err: AssetError): I18nMessage {
  switch (err.code) {
    case "InvalidDateFormat":
      return { key: "error.InvalidDateFormat", vars: { date: err.date } };
    case "AssetNotFound":
    case "Archived":
    case "DatabaseError":
    case "PriceNotFound":
    case "NotPositive":
    case "NonFinite":
    case "DateInFuture":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}

/**
 * F27 — Maps the AssetError variants reachable by the price-refresh lock
 * toggle commands (MKT-156) to an i18n key. The reachable codes per the asset
 * contract are `AssetNotFound`, `CashAssetNotEditable`, `DatabaseError`; other
 * `AssetError` variants cannot be produced by `block_asset_price_refresh` /
 * `unblock_asset_price_refresh` so they map to a generic key.
 */
export function priceRefreshLockErrorToI18n(err: AssetError): I18nMessage {
  switch (err.code) {
    case "AssetNotFound":
    case "CashAssetNotEditable":
    case "DatabaseError":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}

/**
 * MKT-196 — Maps the `backfill_holding_price_history` rejections to i18n keys: the
 * backfill's own causes under `mkt.backfill.error`, the shared ones under `error`.
 */
export function priceHistoryBackfillErrorToI18n(err: PriceHistoryBackfillError): I18nMessage {
  switch (err.code) {
    case "AssetNeverHeld":
    case "PriceRefreshBlocked":
    case "TickerNotResolved":
    case "ProviderUnreachable":
      return { key: `mkt.backfill.error.${err.code}` };
    case "AccountNotFound":
    case "AssetNotFound":
    case "CashAssetNotEditable":
    case "Archived":
    case "DatabaseError":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}

/** MKT-197 — the snackbar message for a backfill that succeeded. */
export function priceHistoryBackfillOutcomeToMessage(
  outcome: PriceHistoryBackfillOutcome,
): I18nMessage & { variant: "success" | "info" } {
  if (outcome.written === 0) {
    return { key: "mkt.backfill.nothing_missing", variant: "info" };
  }
  return {
    key: "mkt.backfill.success",
    vars: { written: outcome.written, already_priced: outcome.already_priced },
    variant: "success",
  };
}

/**
 * F27 — Maps the `record_dividend` error surface to an i18n key. `DividendError`
 * is an untagged union of two tagged leaves (`AccountError` |
 * `DividendTask`) whose combined `code` set is wider than what the
 * command can actually raise, so this switch lists the reachable codes (per the
 * account contract) and falls back to a generic key for any other — mirroring
 * `priceRefreshLockErrorToI18n` rather than the `never`-exhaustive style used
 * for narrow single-type errors.
 */
export function dividendErrorToI18n(err: DividendError): I18nMessage {
  switch (err.code) {
    case "AccountNotFound":
    case "DatabaseError":
    case "AssetNotFound":
    case "AssetNotHeld":
    case "DividendOnCashAsset":
    case "AmountNotPositive":
    case "ExchangeRateNotPositive":
    case "DateInFuture":
    case "DateTooOld":
    case "InvalidDate":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}

/**
 * F27 — Maps the free-shares error surfaces to an i18n key (FSD-021/011/040).
 * Covers both the create path (`FreeSharesError`) and the edit path
 * (`AccountError`, via `correct_transaction`). Every reachable code resolves to
 * `error.{code}`; the two holding-internal codes that never reach the wire
 * (`NegativeQuantity`, `NegativeAveragePrice`) have no i18n key, so they fall
 * back to `error.Unknown`.
 */
export function freeSharesErrorToI18n(err: FreeSharesError | AccountError): I18nMessage {
  if (err.code === "NegativeQuantity" || err.code === "NegativeAveragePrice") {
    return { key: "error.Unknown" };
  }
  return { key: `error.${err.code}` };
}

/**
 * F27 — Maps the interest error surfaces to an i18n key (INT-021/011/040).
 * Covers both the create path (`InterestError`) and the edit path
 * (`AccountError`, via `correct_transaction`). Every reachable code resolves to
 * `error.{code}`; the two holding-internal codes that never reach the wire
 * (`NegativeQuantity`, `NegativeAveragePrice`) have no i18n key, so they fall
 * back to `error.Unknown`.
 */
export function interestErrorToI18n(err: InterestError | AccountError): I18nMessage {
  if (err.code === "NegativeQuantity" || err.code === "NegativeAveragePrice") {
    return { key: "error.Unknown" };
  }
  return { key: `error.${err.code}` };
}

/**
 * F27 — Maps the split error surfaces to an i18n key (SPL-011/012/021/030).
 * Covers both the create path (`SplitError`) and the edit path
 * (`AccountError`, via `correct_transaction`). Every reachable code resolves to
 * `error.{code}`; the two holding-internal codes that never reach the wire
 * (`NegativeQuantity`, `NegativeAveragePrice`) have no i18n key, so they fall
 * back to `error.Unknown`.
 */
export function splitErrorToI18n(err: SplitError | AccountError): I18nMessage {
  if (err.code === "NegativeQuantity" || err.code === "NegativeAveragePrice") {
    return { key: "error.Unknown" };
  }
  return { key: `error.${err.code}` };
}

/**
 * F27 — Maps the management-fee error surfaces to an i18n key (FEE-021/011/027).
 * Covers the one-off record path (`ManagementFeeError`, which folds the cross-BC
 * `AssetNotFound` / `AssetNotHeld` / `ManagementFeeOnCashAsset` checks together
 * with account-BC rejections) and the schedule-CRUD path (`AccountError`). Every
 * reachable code resolves to `error.{code}`.
 */
export function managementFeeErrorToI18n(err: ManagementFeeError | AccountError): I18nMessage {
  // FEE-028 — the rejection carries the quantity held as of the fee's date.
  if (err.code === "ResultingQuantityNotBelowHeld") {
    return {
      key: "error.ResultingQuantityNotBelowHeld",
      vars: { held: microToFormattedQuantity(err.held_quantity) },
    };
  }
  return { key: `error.${err.code}` };
}

/**
 * F27 — Maps the holding-note error surface to an i18n key (HNO-011/020/021).
 * Both commands (`upsert_holding_note` / `delete_holding_note`) reject with the
 * BC-wide `AccountError`. Every reachable code resolves to `error.{code}`; the
 * two holding-internal codes that never reach the wire (`NegativeQuantity`,
 * `NegativeAveragePrice`) have no i18n key, so they fall back to `error.Unknown`.
 */
export function holdingNoteErrorToI18n(err: AccountError): I18nMessage {
  if (err.code === "NegativeQuantity" || err.code === "NegativeAveragePrice") {
    return { key: "error.Unknown" };
  }
  return { key: `error.${err.code}` };
}

const DASH = "—";

export interface HoldingRowViewModel {
  assetId: string;
  assetName: string;
  assetReference: string;
  /** ISO 4217 currency code of the asset (MKT-023). */
  assetCurrency: string;
  quantity: string;
  /** Holding quantity in raw micro-units — used to pass to SellTransactionModal (SEL-010). */
  quantityMicro: number;
  averagePrice: string;
  /** Market value = current_price × quantity, asset currency; "—" when no price (MKT-143). */
  currentValue: string;
  /** Formatted realized P&L string (2 decimal places, SEL-042). */
  realizedPnl: string;
  /** Raw realized P&L in micro-units — used for sign-based color styling (SEL-043). */
  realizedPnlRaw: number;
  /** Always true — active holding rows can trigger the price entry modal (MKT-010). */
  canEnterPrice: boolean;
  /** Current price state — formatted price or typed diagnostic (MKT-030, MKT-032). */
  currentPrice: CurrentPriceState;
  /** ISO date of the price observation, or null when no price recorded (MKT-030). */
  currentPriceDate: string | null;
  /** Formatted unrealized P&L (2 decimal places) or "—" when not computable (MKT-032/034). */
  unrealizedPnl: string;
  /** Raw unrealized P&L in micro-units, or null when not computable (MKT-034). */
  unrealizedPnlRaw: number | null;
  /** Formatted performance % (e.g. "5.25%") or "—" when not computable (MKT-032/035). */
  performancePct: string;
  /** Windowed performance % per window (ACD-054), formatted like performancePct; "—" when the window is not computable (ACD-057), "" for the cash row. */
  periodPerformance: Record<PerformanceWindow, PerformancePctCell>;
  /** Formatted cumulative dividends received for this holding, account currency (DIV-072). Always shown ("0,00" when none). */
  dividendsReceived: string;
  /** Formatted cumulative management fees deducted for this holding, account currency (FEE-052). Always shown ("0,00" when none). */
  managementFees: string;
  /** Formatted weight of the holding in the account's Global Value (e.g. "12.34%"), or "—" when the holding has no market value or the Global Value is 0 (ACD-052). */
  weightPct: string;
  /** Formatted annual rate of the active fee schedule (e.g. "1,50%"), or null when no active schedule exists (FEE-074). */
  feeRatePct: string | null;
  /** Formatted total return % (price + dividends) or "—" when not computable (DIV-071/072). */
  totalReturnPct: string;
  /** Raw total return % in micro-units, or null when not computable — used for sign-based color styling (DIV-072). */
  totalReturnPctRaw: number | null;
  /** True when this row is the system Cash Holding (CSH-090). Drives the cash variant in HoldingRow. */
  isCash: boolean;
  /** Staleness label for the current price (MKT-140); null when no price is recorded. */
  staleness: StalenessLabel | null;
  /** i18n key for the price source badge (MKT-142), or null when no price is recorded. */
  sourceLabel: string | null;
  /** FX-rate staleness label (FXR-090); null/absent unless a converted value is shown. */
  fxStaleness?: StalenessLabel | null;
  /** Text of the note pinned to this holding, or null when none exists (HNO-041). */
  noteText: string | null;
  /** True when the note carries a price alarm — drives the bell icon (HNO-041). */
  noteHasAlarm: boolean;
  /** True while the alarm's threshold is crossed — filled error-tone bell (HNO-030/041). */
  noteAlarmTriggered: boolean;
  /** Derived after a merge: oversold quantity or overdrawn cash, or null when consistent (SYN-040, CFR-042). */
  inconsistency: HoldingInconsistency | null;
  /** Display message for `inconsistency`, amount formatted; null when consistent. */
  inconsistencyLabel: I18nMessage | null;
}

/** The five windowed returns of the period selector — every period except the since-start default (ACD-054). */
export type PerformanceWindow = Exclude<StoredPerfPeriod, "since_start">;

/** One Performance-% cell: the display string plus the raw sign source for gain/loss coloring. */
export interface PerformancePctCell {
  /** Formatted percentage (e.g. "5,25%"), "—" when not computable, "" for the cash row. */
  formatted: string;
  /** Raw micro-value driving the gain/loss color; null when not computable. */
  raw: number | null;
}

/**
 * MKT-032 — Discriminated state for the Current Price cell. When `current_price`
 * is `None`, the cell renders a typed diagnostic so the user can see _why_ a
 * price is unavailable (instead of an unannotated "—").
 *
 * - `present`: a recorded price; `formatted` holds the displayable string.
 * - `missing_ticker`: `asset_reference` is empty — adding a ticker would unlock fetch.
 * - `no_price_available`: reference is non-empty but no price recorded (provider
 *   returned N/D, or no fetch has run yet — intentionally merged per spec).
 *
 * Cash-row variant uses `{ kind: "present", formatted: "" }`; the HoldingRow
 * cash branch never reads this field (renders empty cells).
 */
export type CurrentPriceState =
  | { kind: "present"; formatted: string }
  | { kind: "missing_ticker" }
  | { kind: "no_price_available" };

export interface ClosedHoldingRowViewModel {
  assetId: string;
  assetName: string;
  assetReference: string;
  /** Formatted realized P&L string (2 decimal places, ACD-049). */
  realizedPnl: string;
  /** Raw realized P&L in micro-units — used for sign-based color styling (ACD-049). */
  realizedPnlRaw: number;
  /** Formatted dividends received over the life of the position (2 decimals, DIV-073). */
  dividendsReceived: string;
  /** Raw dividends received in micro-units (DIV-073). */
  dividendsReceivedRaw: number;
  /** Formatted total revenues = realized P&L + dividends (2 decimals). */
  totalRevenues: string;
  /** Raw total revenues in micro-units — used for sign-based color styling. */
  totalRevenuesRaw: number;
  /** ISO date of last sell "YYYY-MM-DD" (ACD-049). */
  lastSoldDate: string;
}

export interface AccountSummaryViewModel {
  accountName: string;
  holdingCount: number;
  isEmpty: boolean;
  isAllClosed: boolean;
  /** True when there is at least one closed holding to display (ACD-048). */
  hasClosedHoldings: boolean;
  /** Formatted total Global Value (cash + priced holdings, 2 decimals, CSH-094). */
  totalGlobalValue: string;
  /** Raw total Global Value in micro-units (CSH-094). */
  totalGlobalValueRaw: number;
  /** True when the account currently holds a non-zero cash balance (CSH-019/095). */
  hasCashHolding: boolean;
  /** Formatted sum of management fees across all active holdings, account currency (FEE-053). */
  totalManagementFees: string;
  /** Raw total management fees in micro-units (FEE-053). */
  totalManagementFeesRaw: number;
  /** Formatted net cash input since inception: deposits − withdrawals, account currency (ACD-053). */
  totalNetCashInput: string;
}

/**
 * MKT-140 — i18n descriptor for the staleness of the current price, via the
 * shared `formatStalenessLabel` with the price-specific `mkt.staleness_*` keys.
 */
export function formatStaleness(
  currentPriceDate: string | null,
  today: Date,
): StalenessLabel | null {
  return formatStalenessLabel(currentPriceDate, today, {
    today: "mkt.staleness_today",
    daysAgo: "mkt.staleness_days_ago",
  });
}

/**
 * FXR-090 — Staleness label for the FX rate used to value a foreign holding in
 * the account currency. `null` when no FX rate date is present (same-currency,
 * no usable rate, or cash). Emits the `currency.rate_staleness_*` i18n keys
 * shared with the currency feature (key strings only — no cross-feature import).
 */
export function formatFxStaleness(fxRateDate: string | null, today: Date): StalenessLabel | null {
  return formatStalenessLabel(fxRateDate, today, {
    today: "currency.rate_staleness_today",
    daysAgo: "currency.rate_staleness_days_old",
  });
}

/**
 * MKT-032 — Derives the Current Price cell state from a holding's data.
 * Pure function: no i18n; price formatting delegates to `microToFormattedPrice`.
 */
export function derivePriceState(detail: HoldingDetail): CurrentPriceState {
  if (detail.current_price !== null) {
    return { kind: "present", formatted: microToFormattedPrice(detail.current_price) };
  }
  if (detail.asset_reference.trim() === "") {
    return { kind: "missing_ticker" };
  }
  return { kind: "no_price_available" };
}

/**
 * MKT-141 / MKT-142 — Maps an AssetPriceSource to its i18n label key, or null when source is null.
 */
export function formatSource(source: AssetPriceSource | null): string | null {
  if (source === null) return null;
  switch (source) {
    case "Manual":
      return "mkt.source_manual";
    case "YahooFinance":
      return "mkt.source_yahoo";
  }
}

/**
 * MKT-011 — the active non-cash holdings selectable when recording a price, in
 * the shape the price modal's asset combobox consumes.
 */
export function toPriceableAssets(holdings: HoldingRowViewModel[]): PriceableAsset[] {
  return holdings
    .filter((h) => h.canEnterPrice)
    .map((h) => ({
      assetId: h.assetId,
      assetName: h.assetName,
      assetCurrency: h.assetCurrency,
    }));
}

/**
 * ACD-052 — weight of a holding in the account's Global Value, formatted as a
 * percentage with 2 decimals. "—" when the holding carries no market value
 * (unpriced / no usable FX rate) or when the Global Value is not positive.
 */
function formatWeightPct(marketValue: number | null, totalGlobalValue: number): string {
  if (marketValue === null || totalGlobalValue <= 0) return DASH;
  const weightPctMicros = Math.round((marketValue / totalGlobalValue) * 100_000_000);
  return `${microToFormatted(weightPctMicros, 2)}%`;
}

function formatPerformancePctCell(valueMicroPercent: number | null): PerformancePctCell {
  return {
    formatted: valueMicroPercent !== null ? `${microToFormatted(valueMicroPercent, 2)}%` : DASH,
    raw: valueMicroPercent,
  };
}

/** ACD-054 — the windowed returns, each formatted through the performance-% pipeline. */
function toPeriodPerformance(
  periodPerformance: HoldingPeriodPerformance,
): Record<PerformanceWindow, PerformancePctCell> {
  return {
    ytd: formatPerformancePctCell(periodPerformance.ytd),
    one_year: formatPerformancePctCell(periodPerformance.one_year),
    two_years: formatPerformancePctCell(periodPerformance.two_years),
    five_years: formatPerformancePctCell(periodPerformance.five_years),
    ten_years: formatPerformancePctCell(periodPerformance.ten_years),
  };
}

// Cash-row variant: the Performance % cell renders blank regardless of the
// selected window, mirroring performancePct's empty string (CSH-090, ACD-054).
const CASH_PERFORMANCE_CELL: PerformancePctCell = { formatted: "", raw: null };
const CASH_PERIOD_PERFORMANCE: Record<PerformanceWindow, PerformancePctCell> = {
  ytd: CASH_PERFORMANCE_CELL,
  one_year: CASH_PERFORMANCE_CELL,
  two_years: CASH_PERFORMANCE_CELL,
  five_years: CASH_PERFORMANCE_CELL,
  ten_years: CASH_PERFORMANCE_CELL,
};

/**
 * ACD-054 — the Performance-% cell for the selected period: "since_start" keeps
 * the existing since-inception figure (colored by the unrealized-P&L sign,
 * MKT-035); the windowed periods read the matching Simple Dietz return.
 */
export function selectPerformanceCell(
  row: HoldingRowViewModel,
  period: StoredPerfPeriod,
): PerformancePctCell {
  if (period === "since_start") {
    return { formatted: row.performancePct, raw: row.unrealizedPnlRaw };
  }
  return row.periodPerformance[period];
}

/** ACD-054 — i18n key for the Performance-% column header, reflecting the selected period. */
export function performanceColumnKey(period: StoredPerfPeriod): string {
  return period === "since_start"
    ? "account_details.column_performance_pct"
    : `account_details.column_performance_pct_${period}`;
}

// reviewer-frontend FP: the default is deliberate — weight legitimately dashes when
// no account total is supplied; the wiring is covered by the useAccountDetails
// ACD-052 test — next-batch 2026-07-04
export function toHoldingRow(detail: HoldingDetail, totalGlobalValue = 0): HoldingRowViewModel {
  const isCash = isCashAsset(detail.asset_id);
  if (isCash) {
    // Cash row variant (CSH-090/091): no cost basis, average price, realized PnL or
    // market-price columns — those cells render blank in the table. Quantity is the
    // running cash balance, formatted to 2 decimals like an amount.
    return {
      assetId: detail.asset_id,
      assetName: detail.asset_name,
      assetReference: detail.asset_reference,
      assetCurrency: detail.asset_currency,
      quantity: microToFormatted(detail.quantity, 2),
      quantityMicro: detail.quantity,
      averagePrice: "",
      currentValue: "",
      realizedPnl: "",
      realizedPnlRaw: 0,
      canEnterPrice: false,
      currentPrice: { kind: "present", formatted: "" },
      currentPriceDate: null,
      unrealizedPnl: "",
      unrealizedPnlRaw: null,
      performancePct: "",
      periodPerformance: CASH_PERIOD_PERFORMANCE,
      dividendsReceived: "",
      managementFees: "",
      weightPct: formatWeightPct(detail.market_value, totalGlobalValue),
      feeRatePct: null,
      totalReturnPct: "",
      totalReturnPctRaw: null,
      isCash: true,
      staleness: null,
      sourceLabel: null,
      fxStaleness: null,
      // HNO-011 — the cash line never carries a note (NoteOnCashAsset).
      noteText: null,
      noteHasAlarm: false,
      noteAlarmTriggered: false,
      inconsistency: detail.inconsistency,
      inconsistencyLabel: toInconsistencyLabel(detail.inconsistency),
    };
  }
  return {
    assetId: detail.asset_id,
    assetName: detail.asset_name,
    assetReference: detail.asset_reference,
    assetCurrency: detail.asset_currency,
    quantity: microToFormattedQuantity(detail.quantity),
    quantityMicro: detail.quantity,
    averagePrice: microToFormattedPrice(detail.average_price),
    // MKT-143 — market value = current_price × quantity in the asset's native
    // currency; "—" when no price has been recorded. Dividing the price out of
    // micros before multiplying keeps the intermediate below MAX_SAFE_INTEGER
    // (vs price_micros × qty_micros); the sub-micro float drift is absorbed by
    // microToFormatted's 2-decimal rounding.
    currentValue:
      detail.current_price !== null
        ? microToFormatted((detail.current_price / 1_000_000) * detail.quantity, 2)
        : DASH,
    realizedPnl: microToFormatted(detail.realized_pnl, 2),
    realizedPnlRaw: detail.realized_pnl,
    canEnterPrice: true,
    currentPrice: derivePriceState(detail),
    currentPriceDate: detail.current_price_date,
    unrealizedPnl:
      detail.unrealized_pnl !== null ? microToFormatted(detail.unrealized_pnl, 2) : DASH,
    unrealizedPnlRaw: detail.unrealized_pnl,
    performancePct:
      detail.performance_pct !== null ? `${microToFormatted(detail.performance_pct, 2)}%` : DASH,
    periodPerformance: toPeriodPerformance(detail.period_performance),
    dividendsReceived: microToFormatted(detail.dividends_received, 2),
    managementFees: microToFormatted(detail.management_fees, 2),
    weightPct: formatWeightPct(detail.market_value, totalGlobalValue),
    // FEE-074 — annual rate of the active fee schedule, shown next to the fees.
    feeRatePct:
      detail.fee_rate_percent_micros !== null
        ? `${microToFormatted(detail.fee_rate_percent_micros, 2)}%`
        : null,
    totalReturnPct:
      detail.total_return_pct !== null ? `${microToFormatted(detail.total_return_pct, 2)}%` : DASH,
    totalReturnPctRaw: detail.total_return_pct,
    isCash: false,
    staleness: formatStaleness(detail.current_price_date, new Date()),
    sourceLabel: formatSource(detail.current_price_source),
    // FXR-090 — staleness of the FX rate used to value this holding; null for
    // same-currency / no-rate holdings (fx_rate_date is None).
    fxStaleness: formatFxStaleness(detail.fx_rate_date, new Date()),
    // HNO-041 — note line under the asset name; the alarm fields come as a pair
    // (both or neither, HNO-011), so the threshold alone signals an alarm.
    noteText: detail.note_text,
    noteHasAlarm: detail.note_threshold_price !== null,
    noteAlarmTriggered: detail.note_alarm_triggered,
    inconsistency: detail.inconsistency,
    inconsistencyLabel: toInconsistencyLabel(detail.inconsistency),
  };
}

function toInconsistencyLabel(reason: HoldingInconsistency | null): I18nMessage | null {
  return reason === null ? null : formatHoldingInconsistency(reason);
}

export function toClosedHoldingRow(detail: ClosedHoldingDetail): ClosedHoldingRowViewModel {
  return {
    assetId: detail.asset_id,
    assetName: detail.asset_name,
    assetReference: detail.asset_reference,
    realizedPnl: microToFormatted(detail.realized_pnl, 2),
    realizedPnlRaw: detail.realized_pnl,
    dividendsReceived: microToFormatted(detail.dividends_received, 2),
    dividendsReceivedRaw: detail.dividends_received,
    totalRevenues: microToFormatted(detail.realized_pnl + detail.dividends_received, 2),
    totalRevenuesRaw: detail.realized_pnl + detail.dividends_received,
    lastSoldDate: detail.last_sold_date,
  };
}

export function toAccountSummary(response: AccountDetailsResponse): AccountSummaryViewModel {
  // CSH-098 — exclude the cash holding from the active count used for empty/all-closed
  // gating, otherwise an account with only a cash holding would never trigger the
  // "no positions yet" empty state nor the closed-only message.
  const nonCashActive = response.holdings.filter((h) => !isCashAsset(h.asset_id));
  const hasCashHolding = response.holdings.some((h) => isCashAsset(h.asset_id) && h.quantity > 0);
  return {
    accountName: response.account_name,
    holdingCount: response.total_holding_count,
    // CSH-098 — cash is excluded from both counts. With eager cash, every account
    // has a Cash Holding (total_holding_count >= 1), so these key off non-cash active
    // holdings + closed holdings only, never the raw count.
    isEmpty: nonCashActive.length === 0 && response.closed_holdings.length === 0,
    isAllClosed: nonCashActive.length === 0 && response.closed_holdings.length > 0,
    hasClosedHoldings: response.closed_holdings.length > 0,
    totalGlobalValue: microToFormatted(response.total_global_value, 2),
    totalGlobalValueRaw: response.total_global_value,
    hasCashHolding,
    totalManagementFees: microToFormatted(response.total_management_fees, 2),
    totalManagementFeesRaw: response.total_management_fees,
    totalNetCashInput: microToFormatted(response.total_net_cash_input, 2),
  };
}
