import type { CurrencyError, CurrencyRateSource, RateHistoryBackfillError } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";
import { formatStalenessLabel, type StalenessLabel } from "@/ui/format/staleness";

/**
 * F27 — Maps a `RateHistoryBackfillError` (FXR-110/114) to an i18n key.
 * Reuses the `error.currency.*` namespace — both codes describe the same
 * bounded context's failures.
 */
export function rateHistoryBackfillErrorToI18n(error: RateHistoryBackfillError): I18nMessage {
  switch (error.code) {
    case "ProviderUnreachable":
    case "DatabaseError":
      return { key: `error.currency.${error.code}` };
    default: {
      const _exhaustive: never = error;
      return _exhaustive;
    }
  }
}

/**
 * F27 — Maps a `CurrencyError` to an i18n key (+ optional interpolation vars).
 * Pure: no React, no `useTranslation`. The exhaustive switch on `code` lets
 * TypeScript catch any new variant at compile time. Keys live under the
 * `error.currency.*` namespace so they don't collide with the shared
 * `error.*` codes used by other bounded contexts.
 */
export function currencyErrorToI18n(error: CurrencyError): I18nMessage {
  switch (error.code) {
    case "InvalidDateFormat":
      return { key: "error.currency.InvalidDateFormat", vars: { date: error.date } };
    case "NotPositive":
    case "NonFinite":
    case "DateInFuture":
    case "InvalidCurrency":
    case "IdentityPair":
    case "RateNotFound":
    case "ProviderUnreachable":
    case "DatabaseError":
      return { key: `error.currency.${error.code}` };
    default: {
      const _exhaustive: never = error;
      return _exhaustive;
    }
  }
}

/**
 * Formats an i64-micros rate (ADR-001) as a human-readable decimal string.
 * Always shows at least 2 decimal places; trailing zeros beyond the 2nd are
 * trimmed but never below 2 (so `1_000_000` → `"1.00"`, `920_000` → `"0.920000"`).
 */
export function formatRateMicros(rateMicros: number): string {
  const value = rateMicros / 1_000_000;
  // Sub-unit rates (e.g. 0.92) keep full micro precision so small differences
  // stay visible; rates ≥ 1 collapse to a 2-decimal display.
  if (Math.abs(value) < 1) {
    return value.toFixed(6);
  }
  return value.toFixed(2);
}

/**
 * FXR-090 — i18n descriptor for how stale the latest rate is, via the shared
 * `formatStalenessLabel` with the `currency.rate_staleness_*` keys.
 */
export function formatRateStaleness(rateDate: string | null, today: Date): StalenessLabel | null {
  return formatStalenessLabel(rateDate, today, {
    today: "currency.rate_staleness_today",
    daysAgo: "currency.rate_staleness_days_old",
  });
}

/**
 * FXR-102 — Maps a `CurrencyRateSource` to its i18n badge key, or `null` when
 * no source is present (no rate recorded yet).
 */
export function formatRateSource(source: CurrencyRateSource | null): string | null {
  if (source === null) return null;
  switch (source) {
    case "Manual":
      return "currency.source_manual";
    case "Frankfurter":
      return "currency.source_frankfurter";
    case "Ecb":
      return "currency.source_ecb";
  }
}

/** Raw string inputs collected by the record/edit rate form. */
export interface RateFormInput {
  fromCurrency: string;
  toCurrency: string;
  date: string;
  rate: string;
}

/** Per-field validation errors keyed by form field. */
export interface RateFormErrors {
  fromCurrency?: string;
  toCurrency?: string;
  date?: string;
  rate?: string;
}

export interface RateFormValidation {
  isValid: boolean;
  errors: RateFormErrors;
}

const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

/**
 * FXR-020/021/022/023 — Inline validation for the record/edit rate form.
 * Mirrors the domain guards so the user gets immediate feedback before the
 * round-trip. The optional `today` lets tests pin "now"; defaults to the
 * current date.
 */
export function validateRateForm(
  input: RateFormInput,
  today: Date = new Date(),
): RateFormValidation {
  const errors: RateFormErrors = {};

  // FXR-020 — all fields required.
  if (input.fromCurrency.trim() === "") errors.fromCurrency = "currency.error.from_required";
  if (input.toCurrency.trim() === "") errors.toCurrency = "currency.error.to_required";
  if (input.date.trim() === "") errors.date = "currency.error.date_required";
  if (input.rate.trim() === "") errors.rate = "currency.error.rate_required";

  // FXR-023 — from and to must differ.
  if (
    input.fromCurrency.trim() !== "" &&
    input.fromCurrency.trim().toUpperCase() === input.toCurrency.trim().toUpperCase()
  ) {
    errors.toCurrency = "currency.error.identity_pair";
  }

  // FXR-021 — rate strictly positive.
  if (input.rate.trim() !== "") {
    const parsed = Number(input.rate);
    if (!Number.isFinite(parsed) || parsed <= 0) {
      errors.rate = "currency.error.rate_not_positive";
    }
  }

  // FXR-022 — date well-formed and not in the future.
  if (input.date.trim() !== "") {
    if (!ISO_DATE.test(input.date.trim())) {
      errors.date = "currency.error.invalid_date";
    } else {
      const observed = new Date(`${input.date.trim()}T00:00:00`);
      const startOfToday = new Date(today.getFullYear(), today.getMonth(), today.getDate());
      if (Number.isNaN(observed.getTime())) {
        errors.date = "currency.error.invalid_date";
      } else if (observed.getTime() > startOfToday.getTime()) {
        errors.date = "currency.error.date_in_future";
      }
    }
  }

  return { isValid: Object.keys(errors).length === 0, errors };
}
