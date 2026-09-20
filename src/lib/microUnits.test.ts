import { describe, expect, it } from "vitest";
import { deriveUnitPriceMicro } from "./microUnits";

const MICRO = 1_000_000;

describe("deriveUnitPriceMicro", () => {
  // TRX-060 pin — 1000.000001 total for 3 units does not divide evenly:
  // 1_000_000_001 / 3 = 333_333_333.67 → rounds to 333_333_334 (same as backend).
  it("rounds the non-terminating 3-unit / 1000.000001 case like the backend", () => {
    expect(deriveUnitPriceMicro(1_000_000_001, 0, 3 * MICRO, MICRO, false)).toBe(333_333_334);
  });

  // TRX-060 — buy: fees are deducted from the total before derivation.
  it("deducts fees before deriving on a buy", () => {
    expect(deriveUnitPriceMicro(210 * MICRO, 10 * MICRO, 2 * MICRO, MICRO, false)).toBe(
      100 * MICRO,
    );
  });

  // SEL-050 — sell: fees are added back to the net proceeds before derivation.
  it("adds fees back before deriving on a sell", () => {
    expect(deriveUnitPriceMicro(190 * MICRO, 10 * MICRO, 2 * MICRO, MICRO, true)).toBe(100 * MICRO);
  });

  // TRX-060 — the exchange rate divides the account-currency amount back to asset currency.
  it("applies the exchange rate in the denominator", () => {
    // 220 account-currency for 1 unit at rate 1.1 → 200 in asset currency
    expect(deriveUnitPriceMicro(220 * MICRO, 0, 1 * MICRO, 1_100_000, false)).toBe(200 * MICRO);
  });

  // TRX-060 — an exact .5 fraction rounds half away from zero.
  it("rounds an exact half away from zero", () => {
    // 1 micro of securities over 2 units → 0.5 micro → 1
    expect(deriveUnitPriceMicro(1, 0, 2 * MICRO, MICRO, false)).toBe(1);
  });

  it("rounds a negative exact half away from zero", () => {
    // total 0, fees 1 micro on a buy → securities −1 micro over 2 units → −0.5 → −1
    expect(deriveUnitPriceMicro(0, 1, 2 * MICRO, MICRO, false)).toBe(-1);
  });

  it("returns 0 when quantity is zero", () => {
    expect(deriveUnitPriceMicro(100 * MICRO, 0, 0, MICRO, false)).toBe(0);
  });

  it("returns 0 when the exchange rate is zero", () => {
    expect(deriveUnitPriceMicro(100 * MICRO, 0, MICRO, 0, false)).toBe(0);
  });
});
