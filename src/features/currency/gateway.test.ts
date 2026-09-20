import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, expectTypeOf, it, vi } from "vitest";
import type { CurrencyPair, CurrencyPairSummary, CurrencyRate, Event } from "@/bindings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockInvoke = vi.mocked(invoke);

// Import after mock is registered so bindings.ts picks up the mock
const { currencyGateway } = await import("./gateway");

describe("currencyGateway — subscribeToEvents (TD-011)", () => {
  it("passes the callback the generated event discriminant, not a bare string", () => {
    expectTypeOf(currencyGateway.subscribeToEvents)
      .parameter(0)
      .parameter(0)
      .toEqualTypeOf<Event["type"]>();
  });
});

// ---------------------------------------------------------------------------
// declareCurrencyPair
// ---------------------------------------------------------------------------

describe("currencyGateway — declareCurrencyPair", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // FXR-054 — ok pass-through
  it("declareCurrencyPair passes through ok result (FXR-054)", async () => {
    const pair: CurrencyPair = { from_currency: "USD", to_currency: "EUR" };
    mockInvoke.mockResolvedValue(pair);

    const result = await currencyGateway.declareCurrencyPair("USD", "EUR");

    expect(result).toEqual({ status: "ok", data: pair });
    expect(mockInvoke).toHaveBeenCalledWith("declare_currency_pair", {
      fromCurrency: "USD",
      toCurrency: "EUR",
    });
  });

  // FXR-023 / FXR-011 — error pass-through: IdentityPair
  it("declareCurrencyPair passes through IdentityPair error", async () => {
    mockInvoke.mockRejectedValue({ code: "IdentityPair" });

    const result = await currencyGateway.declareCurrencyPair("EUR", "EUR");

    expect(result).toEqual({ status: "error", error: { code: "IdentityPair" } });
  });

  // FXR-023 — error pass-through: InvalidCurrency with payload
  it("declareCurrencyPair passes through InvalidCurrency error with currency payload", async () => {
    mockInvoke.mockRejectedValue({ code: "InvalidCurrency", currency: "XYZ" });

    const result = await currencyGateway.declareCurrencyPair("USD", "XYZ");

    expect(result).toEqual({
      status: "error",
      error: { code: "InvalidCurrency", currency: "XYZ" },
    });
  });

  // infrastructure failure
  it("declareCurrencyPair passes through DatabaseError", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await currencyGateway.declareCurrencyPair("USD", "EUR");

    expect(result).toEqual({ status: "error", error: { code: "DatabaseError" } });
  });
});

// ---------------------------------------------------------------------------
// recordCurrencyRate
// ---------------------------------------------------------------------------

describe("currencyGateway — recordCurrencyRate", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // FXR-025 — ok pass-through
  it("recordCurrencyRate passes through ok result (FXR-025)", async () => {
    const rate: CurrencyRate = {
      from_currency: "USD",
      to_currency: "EUR",
      date: "2026-06-01",
      rate: 920_000,
      source: "Manual",
    };
    mockInvoke.mockResolvedValue(rate);

    const result = await currencyGateway.recordCurrencyRate("USD", "EUR", "2026-06-01", 0.92);

    expect(result).toEqual({ status: "ok", data: rate });
    expect(mockInvoke).toHaveBeenCalledWith("record_currency_rate", {
      fromCurrency: "USD",
      toCurrency: "EUR",
      date: "2026-06-01",
      rate: 0.92,
    });
  });

  // FXR-021 — NotPositive error
  it("recordCurrencyRate passes through NotPositive error", async () => {
    mockInvoke.mockRejectedValue({ code: "NotPositive" });

    const result = await currencyGateway.recordCurrencyRate("USD", "EUR", "2026-06-01", -0.5);

    expect(result).toEqual({ status: "error", error: { code: "NotPositive" } });
  });

  // FXR-021 — NonFinite error
  it("recordCurrencyRate passes through NonFinite error", async () => {
    mockInvoke.mockRejectedValue({ code: "NonFinite" });

    const result = await currencyGateway.recordCurrencyRate("USD", "EUR", "2026-06-01", Number.NaN);

    expect(result).toEqual({ status: "error", error: { code: "NonFinite" } });
  });

  // FXR-022 — DateInFuture error
  it("recordCurrencyRate passes through DateInFuture error", async () => {
    mockInvoke.mockRejectedValue({ code: "DateInFuture" });

    const result = await currencyGateway.recordCurrencyRate("USD", "EUR", "2099-12-31", 0.92);

    expect(result).toEqual({ status: "error", error: { code: "DateInFuture" } });
  });

  // FXR-022 — InvalidDateFormat with payload
  it("recordCurrencyRate passes through InvalidDateFormat error with date payload", async () => {
    mockInvoke.mockRejectedValue({ code: "InvalidDateFormat", date: "not-a-date" });

    const result = await currencyGateway.recordCurrencyRate("USD", "EUR", "not-a-date", 0.92);

    expect(result).toEqual({
      status: "error",
      error: { code: "InvalidDateFormat", date: "not-a-date" },
    });
  });

  // FXR-023 — InvalidCurrency with payload
  it("recordCurrencyRate passes through InvalidCurrency error", async () => {
    mockInvoke.mockRejectedValue({ code: "InvalidCurrency", currency: "XYZ" });

    const result = await currencyGateway.recordCurrencyRate("USD", "XYZ", "2026-06-01", 0.92);

    expect(result).toEqual({
      status: "error",
      error: { code: "InvalidCurrency", currency: "XYZ" },
    });
  });

  // FXR-011/023 — IdentityPair
  it("recordCurrencyRate passes through IdentityPair error", async () => {
    mockInvoke.mockRejectedValue({ code: "IdentityPair" });

    const result = await currencyGateway.recordCurrencyRate("EUR", "EUR", "2026-06-01", 1.0);

    expect(result).toEqual({ status: "error", error: { code: "IdentityPair" } });
  });

  // infrastructure failure
  it("recordCurrencyRate passes through DatabaseError", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await currencyGateway.recordCurrencyRate("USD", "EUR", "2026-06-01", 0.92);

    expect(result).toEqual({ status: "error", error: { code: "DatabaseError" } });
  });
});

// ---------------------------------------------------------------------------
// updateCurrencyRate
// ---------------------------------------------------------------------------

describe("currencyGateway — updateCurrencyRate", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // FXR-052 — ok pass-through
  it("updateCurrencyRate passes through ok result (FXR-052)", async () => {
    mockInvoke.mockResolvedValue(null);

    const result = await currencyGateway.updateCurrencyRate(
      "USD",
      "EUR",
      "2026-06-01",
      "2026-06-02",
      0.93,
    );

    expect(result).toEqual({ status: "ok", data: null });
    expect(mockInvoke).toHaveBeenCalledWith("update_currency_rate", {
      fromCurrency: "USD",
      toCurrency: "EUR",
      originalDate: "2026-06-01",
      newDate: "2026-06-02",
      newRate: 0.93,
    });
  });

  // FXR-052 — RateNotFound with payload
  it("updateCurrencyRate passes through RateNotFound error with payload", async () => {
    mockInvoke.mockRejectedValue({
      code: "RateNotFound",
      from_currency: "USD",
      to_currency: "EUR",
      date: "2026-06-01",
    });

    const result = await currencyGateway.updateCurrencyRate(
      "USD",
      "EUR",
      "2026-06-01",
      "2026-06-02",
      0.93,
    );

    expect(result).toEqual({
      status: "error",
      error: {
        code: "RateNotFound",
        from_currency: "USD",
        to_currency: "EUR",
        date: "2026-06-01",
      },
    });
  });

  // FXR-021 — NotPositive
  it("updateCurrencyRate passes through NotPositive error", async () => {
    mockInvoke.mockRejectedValue({ code: "NotPositive" });

    const result = await currencyGateway.updateCurrencyRate(
      "USD",
      "EUR",
      "2026-06-01",
      "2026-06-01",
      0,
    );

    expect(result).toEqual({ status: "error", error: { code: "NotPositive" } });
  });

  // FXR-021 — NonFinite
  it("updateCurrencyRate passes through NonFinite error", async () => {
    mockInvoke.mockRejectedValue({ code: "NonFinite" });

    const result = await currencyGateway.updateCurrencyRate(
      "USD",
      "EUR",
      "2026-06-01",
      "2026-06-01",
      Number.POSITIVE_INFINITY,
    );

    expect(result).toEqual({ status: "error", error: { code: "NonFinite" } });
  });

  // FXR-022 — DateInFuture
  it("updateCurrencyRate passes through DateInFuture error", async () => {
    mockInvoke.mockRejectedValue({ code: "DateInFuture" });

    const result = await currencyGateway.updateCurrencyRate(
      "USD",
      "EUR",
      "2026-06-01",
      "2099-12-31",
      0.93,
    );

    expect(result).toEqual({ status: "error", error: { code: "DateInFuture" } });
  });

  // FXR-022 — InvalidDateFormat with payload
  it("updateCurrencyRate passes through InvalidDateFormat error with date payload", async () => {
    mockInvoke.mockRejectedValue({ code: "InvalidDateFormat", date: "bad-date" });

    const result = await currencyGateway.updateCurrencyRate(
      "USD",
      "EUR",
      "2026-06-01",
      "bad-date",
      0.93,
    );

    expect(result).toEqual({
      status: "error",
      error: { code: "InvalidDateFormat", date: "bad-date" },
    });
  });

  // FXR-023 — InvalidCurrency with payload
  it("updateCurrencyRate passes through InvalidCurrency error", async () => {
    mockInvoke.mockRejectedValue({ code: "InvalidCurrency", currency: "XYZ" });

    const result = await currencyGateway.updateCurrencyRate(
      "USD",
      "XYZ",
      "2026-06-01",
      "2026-06-01",
      0.93,
    );

    expect(result).toEqual({
      status: "error",
      error: { code: "InvalidCurrency", currency: "XYZ" },
    });
  });

  // infrastructure failure
  it("updateCurrencyRate passes through DatabaseError", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await currencyGateway.updateCurrencyRate(
      "USD",
      "EUR",
      "2026-06-01",
      "2026-06-01",
      0.93,
    );

    expect(result).toEqual({ status: "error", error: { code: "DatabaseError" } });
  });
});

// ---------------------------------------------------------------------------
// deleteCurrencyRate
// ---------------------------------------------------------------------------

describe("currencyGateway — deleteCurrencyRate", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // FXR-053 — ok pass-through
  it("deleteCurrencyRate passes through ok result (FXR-053)", async () => {
    mockInvoke.mockResolvedValue(null);

    const result = await currencyGateway.deleteCurrencyRate("USD", "EUR", "2026-06-01");

    expect(result).toEqual({ status: "ok", data: null });
    expect(mockInvoke).toHaveBeenCalledWith("delete_currency_rate", {
      fromCurrency: "USD",
      toCurrency: "EUR",
      date: "2026-06-01",
    });
  });

  // FXR-053 — RateNotFound with payload
  it("deleteCurrencyRate passes through RateNotFound error with payload", async () => {
    mockInvoke.mockRejectedValue({
      code: "RateNotFound",
      from_currency: "USD",
      to_currency: "EUR",
      date: "2026-06-01",
    });

    const result = await currencyGateway.deleteCurrencyRate("USD", "EUR", "2026-06-01");

    expect(result).toEqual({
      status: "error",
      error: {
        code: "RateNotFound",
        from_currency: "USD",
        to_currency: "EUR",
        date: "2026-06-01",
      },
    });
  });

  // infrastructure failure
  it("deleteCurrencyRate passes through DatabaseError", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await currencyGateway.deleteCurrencyRate("USD", "EUR", "2026-06-01");

    expect(result).toEqual({ status: "error", error: { code: "DatabaseError" } });
  });
});

// ---------------------------------------------------------------------------
// getCurrencyPairs
// ---------------------------------------------------------------------------

describe("currencyGateway — getCurrencyPairs", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // FXR-051 — ok pass-through
  it("getCurrencyPairs passes through ok result (FXR-051)", async () => {
    const summary: CurrencyPairSummary = {
      from_currency: "USD",
      to_currency: "EUR",
      latest_rate: 920_000,
      latest_rate_date: "2026-06-01",
      latest_rate_source: "Manual",
    };
    mockInvoke.mockResolvedValue([summary]);

    const result = await currencyGateway.getCurrencyPairs();

    expect(result).toEqual({ status: "ok", data: [summary] });
    expect(mockInvoke).toHaveBeenCalledWith("get_currency_pairs");
  });

  // FXR-051 — empty list is a valid ok result (no pairs yet)
  it("getCurrencyPairs passes through empty list as ok", async () => {
    mockInvoke.mockResolvedValue([]);

    const result = await currencyGateway.getCurrencyPairs();

    expect(result).toEqual({ status: "ok", data: [] });
  });

  // infrastructure failure
  it("getCurrencyPairs passes through DatabaseError", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await currencyGateway.getCurrencyPairs();

    expect(result).toEqual({ status: "error", error: { code: "DatabaseError" } });
  });
});

// ---------------------------------------------------------------------------
// getCurrencyRates
// ---------------------------------------------------------------------------

describe("currencyGateway — getCurrencyRates", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // FXR-050 — ok pass-through
  it("getCurrencyRates passes through ok result ordered date descending (FXR-050)", async () => {
    const rates: CurrencyRate[] = [
      {
        from_currency: "USD",
        to_currency: "EUR",
        date: "2026-06-02",
        rate: 921_000,
        source: "Frankfurter",
      },
      {
        from_currency: "USD",
        to_currency: "EUR",
        date: "2026-06-01",
        rate: 920_000,
        source: "Manual",
      },
    ];
    mockInvoke.mockResolvedValue(rates);

    const result = await currencyGateway.getCurrencyRates("USD", "EUR");

    expect(result).toEqual({ status: "ok", data: rates });
    expect(mockInvoke).toHaveBeenCalledWith("get_currency_rates", {
      fromCurrency: "USD",
      toCurrency: "EUR",
    });
  });

  // FXR-050 — empty list for unknown pair (never RateNotFound)
  it("getCurrencyRates passes through empty list for unknown pair", async () => {
    mockInvoke.mockResolvedValue([]);

    const result = await currencyGateway.getCurrencyRates("USD", "EUR");

    expect(result).toEqual({ status: "ok", data: [] });
  });

  // infrastructure failure
  it("getCurrencyRates passes through DatabaseError", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await currencyGateway.getCurrencyRates("USD", "EUR");

    expect(result).toEqual({ status: "error", error: { code: "DatabaseError" } });
  });
});
