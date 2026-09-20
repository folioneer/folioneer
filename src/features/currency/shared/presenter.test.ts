import { describe, expect, it } from "vitest";
import type { CurrencyError, CurrencyRateSource } from "@/bindings";
import {
  currencyErrorToI18n,
  formatRateMicros,
  formatRateSource,
  formatRateStaleness,
  validateRateForm,
} from "./presenter";

// ---------------------------------------------------------------------------
// currencyErrorToI18n — F27 error → i18n key (one test per CurrencyError variant)
// ---------------------------------------------------------------------------

describe("currencyErrorToI18n", () => {
  // Flat variants — code maps to flat key
  it("maps NotPositive to its flat i18n key (FXR-021)", () => {
    const error: CurrencyError = { code: "NotPositive" };
    expect(currencyErrorToI18n(error)).toEqual({ key: "error.currency.NotPositive" });
  });

  it("maps NonFinite to its flat i18n key (FXR-021)", () => {
    const error: CurrencyError = { code: "NonFinite" };
    expect(currencyErrorToI18n(error)).toEqual({ key: "error.currency.NonFinite" });
  });

  it("maps DateInFuture to its flat i18n key (FXR-022)", () => {
    const error: CurrencyError = { code: "DateInFuture" };
    expect(currencyErrorToI18n(error)).toEqual({ key: "error.currency.DateInFuture" });
  });

  // Payload-bearing variant — interpolation required
  it("maps InvalidDateFormat to its i18n key with date interpolation (FXR-022)", () => {
    const error: CurrencyError = { code: "InvalidDateFormat", date: "2026/13/99" };
    expect(currencyErrorToI18n(error)).toEqual({
      key: "error.currency.InvalidDateFormat",
      vars: { date: "2026/13/99" },
    });
  });

  // InvalidCurrency — carries currency payload
  it("maps InvalidCurrency to its flat i18n key (FXR-023)", () => {
    const error: CurrencyError = { code: "InvalidCurrency", currency: "XYZ" };
    expect(currencyErrorToI18n(error)).toEqual({ key: "error.currency.InvalidCurrency" });
  });

  it("maps IdentityPair to its flat i18n key (FXR-011/023)", () => {
    const error: CurrencyError = { code: "IdentityPair" };
    expect(currencyErrorToI18n(error)).toEqual({ key: "error.currency.IdentityPair" });
  });

  // RateNotFound — carries from_currency, to_currency, date payload
  it("maps RateNotFound to its flat i18n key (FXR-052/053)", () => {
    const error: CurrencyError = {
      code: "RateNotFound",
      from_currency: "USD",
      to_currency: "EUR",
      date: "2026-06-01",
    };
    expect(currencyErrorToI18n(error)).toEqual({ key: "error.currency.RateNotFound" });
  });

  it("maps DatabaseError to its flat i18n key", () => {
    const error: CurrencyError = { code: "DatabaseError" };
    expect(currencyErrorToI18n(error)).toEqual({ key: "error.currency.DatabaseError" });
  });
});

// ---------------------------------------------------------------------------
// formatRateMicros — rate micros → human-readable decimal string
// ---------------------------------------------------------------------------

describe("formatRateMicros", () => {
  // 1 000 000 micros = 1.000000 → "1.00" (2 decimal minimum)
  it("formats 1_000_000 micros as 1.00", () => {
    expect(formatRateMicros(1_000_000)).toBe("1.00");
  });

  // 920 000 micros = 0.92 EUR per 1 USD
  it("formats 920_000 micros as 0.920000 (6 decimal places)", () => {
    expect(formatRateMicros(920_000)).toBe("0.920000");
  });

  // Exact 2-decimal value
  it("formats 1_250_000 micros as 1.25", () => {
    expect(formatRateMicros(1_250_000)).toBe("1.25");
  });

  // Large rate (e.g. USD/JPY ~160): 160_000_000 micros
  it("formats 160_000_000 micros as 160.00", () => {
    expect(formatRateMicros(160_000_000)).toBe("160.00");
  });
});

// ---------------------------------------------------------------------------
// formatRateStaleness — FXR-090: "as of today" vs "Nd old"
// ---------------------------------------------------------------------------

describe("formatRateStaleness (FXR-090)", () => {
  const today = new Date("2026-06-02");

  // null date → null (no label)
  it("returns null when rateDate is null", () => {
    expect(formatRateStaleness(null, today)).toBeNull();
  });

  // Same day → "rate as of today" key
  it("returns the today i18n key when rateDate equals today", () => {
    expect(formatRateStaleness("2026-06-02", today)).toEqual({
      key: "currency.rate_staleness_today",
    });
  });

  // 1 day ago → days_old key with days=1
  it("returns the days_old i18n key with days=1 when rateDate is one day before today", () => {
    expect(formatRateStaleness("2026-06-01", today)).toEqual({
      key: "currency.rate_staleness_days_old",
      params: { days: 1 },
    });
  });

  // N days ago → days_old key with days=N
  it("returns the days_old i18n key with days=7 when rateDate is seven days before today", () => {
    expect(formatRateStaleness("2026-05-26", today)).toEqual({
      key: "currency.rate_staleness_days_old",
      params: { days: 7 },
    });
  });
});

// ---------------------------------------------------------------------------
// formatRateSource — FXR-102: source badge label
// ---------------------------------------------------------------------------

describe("formatRateSource (FXR-102)", () => {
  it("returns null when source is null", () => {
    expect(formatRateSource(null)).toBeNull();
  });

  it("maps Manual to the manual i18n key (FXR-101)", () => {
    const source: CurrencyRateSource = "Manual";
    expect(formatRateSource(source)).toBe("currency.source_manual");
  });

  it("maps Frankfurter to the Frankfurter i18n key (FXR-102)", () => {
    const source: CurrencyRateSource = "Frankfurter";
    expect(formatRateSource(source)).toBe("currency.source_frankfurter");
  });

  it("maps Ecb to the ECB i18n key (FXR-102)", () => {
    const source: CurrencyRateSource = "Ecb";
    expect(formatRateSource(source)).toBe("currency.source_ecb");
  });
});

// ---------------------------------------------------------------------------
// validateRateForm — FXR-020/021/022/023: inline form validation
// ---------------------------------------------------------------------------

describe("validateRateForm (FXR-020–023)", () => {
  // FXR-020 — all fields required: submit disabled when any is empty
  it("returns required error when fromCurrency is empty (FXR-020)", () => {
    const result = validateRateForm({
      fromCurrency: "",
      toCurrency: "EUR",
      date: "2026-06-01",
      rate: "0.92",
    });
    expect(result.isValid).toBe(false);
    expect(result.errors.fromCurrency).toBeDefined();
  });

  it("returns required error when toCurrency is empty (FXR-020)", () => {
    const result = validateRateForm({
      fromCurrency: "USD",
      toCurrency: "",
      date: "2026-06-01",
      rate: "0.92",
    });
    expect(result.isValid).toBe(false);
    expect(result.errors.toCurrency).toBeDefined();
  });

  it("returns required error when date is empty (FXR-020)", () => {
    const result = validateRateForm({
      fromCurrency: "USD",
      toCurrency: "EUR",
      date: "",
      rate: "0.92",
    });
    expect(result.isValid).toBe(false);
    expect(result.errors.date).toBeDefined();
  });

  it("returns required error when rate is empty (FXR-020)", () => {
    const result = validateRateForm({
      fromCurrency: "USD",
      toCurrency: "EUR",
      date: "2026-06-01",
      rate: "",
    });
    expect(result.isValid).toBe(false);
    expect(result.errors.rate).toBeDefined();
  });

  // FXR-021 — rate must be > 0
  it("returns rate error when rate is zero (FXR-021)", () => {
    const result = validateRateForm({
      fromCurrency: "USD",
      toCurrency: "EUR",
      date: "2026-06-01",
      rate: "0",
    });
    expect(result.isValid).toBe(false);
    expect(result.errors.rate).toBeDefined();
  });

  it("returns rate error when rate is negative (FXR-021)", () => {
    const result = validateRateForm({
      fromCurrency: "USD",
      toCurrency: "EUR",
      date: "2026-06-01",
      rate: "-0.5",
    });
    expect(result.isValid).toBe(false);
    expect(result.errors.rate).toBeDefined();
  });

  // FXR-022 — date must be ≤ today (not in future)
  it("returns date error when date is in the future (FXR-022)", () => {
    const result = validateRateForm(
      {
        fromCurrency: "USD",
        toCurrency: "EUR",
        date: "2099-12-31",
        rate: "0.92",
      },
      new Date("2026-06-02"),
    );
    expect(result.isValid).toBe(false);
    expect(result.errors.date).toBeDefined();
  });

  it("returns date error when date is not a valid ISO format (FXR-022)", () => {
    const result = validateRateForm({
      fromCurrency: "USD",
      toCurrency: "EUR",
      date: "not-a-date",
      rate: "0.92",
    });
    expect(result.isValid).toBe(false);
    expect(result.errors.date).toBeDefined();
  });

  // FXR-023 — from and to currencies must differ
  it("returns identity error when fromCurrency equals toCurrency (FXR-023)", () => {
    const result = validateRateForm({
      fromCurrency: "EUR",
      toCurrency: "EUR",
      date: "2026-06-01",
      rate: "0.92",
    });
    expect(result.isValid).toBe(false);
    expect(result.errors.toCurrency).toBeDefined();
  });

  // Happy path — all fields valid
  it("returns isValid=true and no errors when all fields are valid", () => {
    const result = validateRateForm(
      {
        fromCurrency: "USD",
        toCurrency: "EUR",
        date: "2026-06-01",
        rate: "0.92",
      },
      new Date("2026-06-02"),
    );
    expect(result.isValid).toBe(true);
    expect(Object.keys(result.errors)).toHaveLength(0);
  });
});
