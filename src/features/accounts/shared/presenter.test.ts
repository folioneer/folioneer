import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { WrapperLists } from "@/bindings";
import { setDisplayLocale } from "@/lib/microUnits";
import {
  accountMutationErrorToI18n,
  countryOptions,
  fetchPriceErrorToI18n,
  formatAccountRowTotalUnrealizedPnl,
  formatAccountRowYtdPerformancePct,
  formatPriceMovementAmount,
  formatPriceMovementDate,
  formatPriceMovementPct,
  formatPriceMovementValue,
  groupByWrapper,
  wrapperLabel,
  wrapperOptions,
} from "./presenter";

// F27 layer-3 presenter — covers the reachable `AccountError` codes for the
// account-BC mutation surface (add / update / delete / deletion-summary), all
// consumed by useAccounts.
describe("accountMutationErrorToI18n", () => {
  it("names the country or the wrapper an account cannot take (ACC-041)", () => {
    expect(accountMutationErrorToI18n({ code: "InvalidCountry", country: "fra" })).toEqual({
      key: "error.InvalidCountry",
      vars: { country: "fra" },
    });
    expect(accountMutationErrorToI18n({ code: "WrapperNotAvailable", wrapper: "de_bav" })).toEqual({
      key: "error.WrapperNotAvailable",
      vars: { wrapper: "de_bav" },
    });
  });

  it("InvalidCurrency interpolates the currency payload", () => {
    expect(accountMutationErrorToI18n({ code: "InvalidCurrency", currency: "ZZZ" })).toEqual({
      key: "error.InvalidCurrency",
      vars: { currency: "ZZZ" },
    });
  });

  it("AccountNotFound maps to its flat key (account_id payload not surfaced)", () => {
    expect(accountMutationErrorToI18n({ code: "AccountNotFound", account_id: "acc-1" })).toEqual({
      key: "error.AccountNotFound",
    });
  });

  it.each([
    "NameEmpty",
    "NameAlreadyExists",
    "DatabaseError",
  ] as const)("%s unit variant maps to its flat error key", (code) => {
    expect(accountMutationErrorToI18n({ code })).toEqual({ key: `error.${code}` });
  });

  it("an unreachable BC-wide code falls back to error.Unknown", () => {
    expect(accountMutationErrorToI18n({ code: "Oversell", available: 1, requested: 2 })).toEqual({
      key: "error.Unknown",
    });
  });
});

// F27 layer-3 presenter — exhaustive variant coverage for fetch-price snackbar
// dispatch. Composes AssetError + AccountError + FetchPriceTask
// (the AssetError contribution is the same DatabaseError code as the account-side).
describe("fetchPriceErrorToI18n", () => {
  it("FetchAlreadyRunning dispatches info snackbar", () => {
    expect(fetchPriceErrorToI18n({ code: "FetchAlreadyRunning" })).toEqual({
      key: "mkt.fetch_already_running",
      severity: "info",
    });
  });

  it("NoFetchableHoldings dispatches info snackbar", () => {
    expect(fetchPriceErrorToI18n({ code: "NoFetchableHoldings" })).toEqual({
      key: "mkt.fetch_no_holdings",
      severity: "info",
    });
  });

  it("AccountNotFound dispatches dedicated error snackbar", () => {
    expect(fetchPriceErrorToI18n({ code: "AccountNotFound", account_id: "acc-1" })).toEqual({
      key: "error.AccountNotFound",
      severity: "error",
    });
  });

  it.each([
    "DatabaseError",
    "UnknownError",
    "NameAlreadyExists",
  ] as const)("%s falls through to the generic DatabaseError snackbar", (code) => {
    expect(fetchPriceErrorToI18n({ code })).toEqual({
      key: "error.DatabaseError",
      severity: "error",
    });
  });

  it("an unreachable BC-wide code falls back to the generic error.Unknown snackbar", () => {
    expect(fetchPriceErrorToI18n({ code: "Oversell", available: 1, requested: 2 })).toEqual({
      key: "error.Unknown",
      severity: "error",
    });
  });
});

// ACC-023 — formatAccountRowTotalUnrealizedPnl: account-currency micros → formatted string
// or "—" when null. Mirrors the HoldingRowViewModel.unrealizedPnl pattern from
// account_details/shared/presenter.ts (microToFormatted with 2 decimals).
describe("formatAccountRowTotalUnrealizedPnl", () => {
  it("returns '—' when total_unrealized_pnl is null", () => {
    expect(formatAccountRowTotalUnrealizedPnl(null)).toBe("—");
  });

  it("formats a positive value (micros) to 2 decimal places", () => {
    // 1_250_000 micros = 1.25 in account currency
    expect(formatAccountRowTotalUnrealizedPnl(1_250_000)).toBe("1,25");
  });

  it("formats a negative value (micros) to 2 decimal places with leading minus", () => {
    // -3_700_000 micros = -3.70 in account currency
    expect(formatAccountRowTotalUnrealizedPnl(-3_700_000)).toBe("-3,70");
  });

  it("formats zero as '0.00'", () => {
    expect(formatAccountRowTotalUnrealizedPnl(0)).toBe("0,00");
  });
});

// ACC-024 — formatAccountRowYtdPerformancePct: micro-percent → signed formatted string
// or "—" when null. 8_000_000 micro-percent = 8.00%, with explicit '+' for positives.
describe("formatAccountRowYtdPerformancePct", () => {
  it("returns '—' when ytd_performance_pct is null", () => {
    expect(formatAccountRowYtdPerformancePct(null)).toBe("—");
  });

  it("formats a positive micro-percent with a leading '+' sign", () => {
    // 8_000_000 micro-percent = 8.00%
    expect(formatAccountRowYtdPerformancePct(8_000_000)).toBe("+8,00%");
  });

  it("formats a negative micro-percent with a leading '-' sign (no explicit '+')", () => {
    // -3_700_000 micro-percent = -3.70%
    expect(formatAccountRowYtdPerformancePct(-3_700_000)).toBe("-3,70%");
  });

  it("formats zero as '+0.00%' (non-negative, treated as positive sign)", () => {
    expect(formatAccountRowYtdPerformancePct(0)).toBe("+0,00%");
  });
});

// PMV-021/022/034/040 — formatPriceMovementValue: account- or reference-currency
// micros → 2-decimal display string, unconverted (the currency itself is a
// separate field the dialog renders alongside it, never combined here).
describe("formatPriceMovementValue", () => {
  // The default display locale is "fr"; these assertions are French-format on
  // purpose. The English case below proves the formatter follows the app
  // language rather than hardcoding one.
  beforeEach(() => setDisplayLocale("fr"));
  afterEach(() => setDisplayLocale("fr"));

  it("formats a positive value (micros) to 2 decimal places", () => {
    // 1_250_000 micros = 1.25
    expect(formatPriceMovementValue(1_250_000)).toBe("1,25");
  });

  it("formats a negative value (micros) to 2 decimal places with leading minus", () => {
    // -3_700_000 micros = -3.70
    expect(formatPriceMovementValue(-3_700_000)).toBe("-3,70");
  });

  it("formats zero as '0.00'", () => {
    expect(formatPriceMovementValue(0)).toBe("0,00");
  });

  // The dialog opens over account rows formatted by the same shared helper —
  // a hardcoded locale here would show comma decimals beside period decimals
  // the moment the user switches language.
  it("follows the app display locale rather than a hardcoded one", () => {
    setDisplayLocale("en");
    expect(formatPriceMovementValue(1_250_000)).toBe("1.25");
  });
});

// PMV-024/031/025/041/044/045 — formatPriceMovementPct: signed micro-percent,
// "—" when the backend reports no proportion (unmoved PMV-031/045, or an
// undefined earlier value PMV-025/044). The presentation never re-derives
// whether a proportion exists — it only renders what it is given.
describe("formatPriceMovementPct", () => {
  beforeEach(() => setDisplayLocale("fr"));
  afterEach(() => setDisplayLocale("fr"));

  it("returns '—' when the movement is null", () => {
    expect(formatPriceMovementPct(null)).toBe("—");
  });

  it("formats a positive micro-percent with a leading '+' sign", () => {
    // 20_000_000 micro-percent = 20.00%
    expect(formatPriceMovementPct(20_000_000)).toBe("+20,00%");
  });

  it("formats a negative micro-percent with a leading '-' sign", () => {
    // -3_700_000 micro-percent = -3.70%
    expect(formatPriceMovementPct(-3_700_000)).toBe("-3,70%");
  });

  it("follows the app display locale rather than a hardcoded one", () => {
    setDisplayLocale("en");
    expect(formatPriceMovementPct(20_000_000)).toBe("+20.00%");
  });
});

// PMV-027/046 — formatPriceMovementAmount: a signed amount with its currency, "—"
// when the backend reports no amount (unmoved PMV-031/045).
describe("formatPriceMovementAmount", () => {
  beforeEach(() => setDisplayLocale("fr"));
  afterEach(() => setDisplayLocale("fr"));

  it("returns '—' when the amount is null", () => {
    expect(formatPriceMovementAmount(null, "EUR")).toBe("—");
  });

  it("formats a gain with a leading '+' sign and the currency", () => {
    expect(formatPriceMovementAmount(857_500_000, "EUR")).toBe("+857,50 EUR");
  });

  it("formats a loss with a leading '-' sign and the currency", () => {
    expect(formatPriceMovementAmount(-20_000_000, "USD")).toBe("-20,00 USD");
  });
});

// PMV-053 — formatPriceMovementDate: a value column's observation date in
// the display locale, "—" when that side of the report carries no date.
describe("formatPriceMovementDate", () => {
  it("returns '—' when the column carries no date", () => {
    expect(formatPriceMovementDate(null, "fr")).toBe("—");
  });

  it("formats the date in the given locale", () => {
    expect(formatPriceMovementDate("2026-09-09", "fr")).toBe("09/09/2026");
    expect(formatPriceMovementDate("2026-09-11", "en")).toBe("9/11/2026");
  });
});

// ACC-045 — the wrapper lists as the window shows them.
const lists: WrapperLists = {
  portfolio_country: "FR",
  wrappers: [
    {
      id: "brokerage",
      country: null,
      label_en: "Brokerage",
      label_fr: "Compte-titres",
      label_de: "Depot",
    },
    { id: "fr_pea", country: "FR", label_en: "PEA", label_fr: "PEA", label_de: "PEA" },
    { id: "de_bav", country: "DE", label_en: "bAV", label_fr: "bAV", label_de: "bAV" },
  ],
  countries: ["FR", "DE"],
  offered: [
    { country: "FR", wrapper_ids: ["fr_pea", "brokerage"] },
    { country: "DE", wrapper_ids: ["de_bav", "brokerage"] },
  ],
  generic: ["brokerage"],
};

describe("wrapperLabel (ACC-045)", () => {
  it("names a wrapper in the window's language, and an unknown one by its identifier", () => {
    expect(wrapperLabel(lists, "brokerage", "fr-FR")).toBe("Compte-titres");
    expect(wrapperLabel(lists, "brokerage", "de")).toBe("Depot");
    expect(wrapperLabel(lists, "brokerage", "en-GB")).toBe("Brokerage");
    expect(wrapperLabel(lists, "uk_isa", "en")).toBe("uk_isa");
    expect(wrapperLabel(lists, null, "en")).toBeNull();
    expect(wrapperLabel(null, "fr_pea", "en")).toBeNull();
  });
});

describe("wrapperOptions (ACC-048)", () => {
  it("offers the country's wrappers first, then the generic ones", () => {
    const ids = (country: string, current: string) =>
      wrapperOptions(lists, country, current, "en").map((o) => o.value);
    expect(ids("", "")).toEqual(["fr_pea", "brokerage"]);
    expect(ids("DE", "")).toEqual(["de_bav", "brokerage"]);
    expect(ids("ES", "")).toEqual(["brokerage"]);
  });

  // ACC-043 — the account's wrapper stays offered under another country, and by its
  // identifier when the lists do not know it, so an edit keeps it.
  it("keeps the account's own wrapper after the suggested ones", () => {
    const options = wrapperOptions(lists, "ES", "fr_pea", "en");
    expect(options.map((o) => o.value)).toEqual(["brokerage", "fr_pea"]);
    expect(options[1]?.label).toBe("PEA");
    const unknown = wrapperOptions(lists, "", "es_plan", "en");
    expect(unknown.map((o) => o.value)).toEqual(["fr_pea", "brokerage", "es_plan"]);
    expect(unknown[2]?.label).toBe("es_plan");
    expect(wrapperOptions(lists, "", "brokerage", "en")).toHaveLength(2);
  });
});

describe("countryOptions (ACC-045)", () => {
  it("offers the portfolio's country first, then each other country with a list", () => {
    const options = countryOptions(lists, "en");
    expect(options.map((o) => o.value)).toEqual(["", "DE"]);
    expect(options[0]?.country).toBe("France");
    expect(options[1]?.label).toBe("Germany");
  });
});

describe("groupByWrapper (ACC-047)", () => {
  it("groups the rows in the lists' order, the accounts without one last", () => {
    const rows = [
      { name: "A", wrapper: "brokerage" },
      { name: "B", wrapper: null },
      { name: "C", wrapper: "fr_pea" },
      { name: "D", wrapper: "brokerage" },
    ];
    const groups = groupByWrapper(rows, lists, (id) => wrapperLabel(lists, id, "en"));
    expect(groups.map((g) => [g.wrapper, g.label, g.rows.map((r) => r.name)])).toEqual([
      ["brokerage", "Brokerage", ["A", "D"]],
      ["fr_pea", "PEA", ["C"]],
      [null, null, ["B"]],
    ]);
  });
});
