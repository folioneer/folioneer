import { describe, expect, it } from "vitest";
import { decimalToMicro } from "./microUnits";

describe("decimalToMicro", () => {
  // TRX-024 — a typed decimal becomes micro-units, with a dot or a comma.
  it("reads a decimal with a dot or a comma", () => {
    expect(decimalToMicro("1.5")).toBe(1_500_000);
    expect(decimalToMicro("1,5")).toBe(1_500_000);
    expect(decimalToMicro(" 12 ")).toBe(12_000_000);
    expect(decimalToMicro("0.000001")).toBe(1);
    expect(decimalToMicro(".5")).toBe(500_000);
  });

  // A minus applies to the whole figure, its decimals included: the core, not this
  // conversion, refuses a negative amount.
  it("keeps the sign of a negative figure", () => {
    expect(decimalToMicro("-10")).toBe(-10_000_000);
    expect(decimalToMicro("-1.5")).toBe(-1_500_000);
    expect(decimalToMicro("-0.5")).toBe(-500_000);
    expect(decimalToMicro("+2.25")).toBe(2_250_000);
  });

  // Text that is no number, and an empty field, read as 0.
  it("reads what is not a number as 0", () => {
    expect(decimalToMicro("")).toBe(0);
    expect(decimalToMicro("abc")).toBe(0);
  });
});
