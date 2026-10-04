import { describe, expect, it } from "vitest";
import { decimalToMicro, microToExactDecimal, microToFieldDecimal } from "./microUnits";

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

// A recorded figure pre-filled in a field must read back to the same micro-units.
describe("microToExactDecimal", () => {
  it("keeps every decimal the value carries and none it does not", () => {
    expect(microToExactDecimal(921_400)).toBe("0.9214");
    expect(microToExactDecimal(18_600_000)).toBe("18.6");
    expect(microToExactDecimal(5_000_000)).toBe("5");
    expect(microToExactDecimal(1)).toBe("0.000001");
    expect(microToExactDecimal(0)).toBe("0");
    expect(microToExactDecimal(-1_250_000)).toBe("-1.25");
  });

  it("reads back to the value it was made from", () => {
    for (const micros of [921_400, 18_600_000, 1, 123_456_789, 1_000_000, -1_250_000]) {
      expect(decimalToMicro(microToExactDecimal(micros))).toBe(micros);
    }
  });
});

// TRX-024 / TD-073 — a field filled from a figure reads back to that figure: every decimal
// the figure carries, and at least three.
describe("microToFieldDecimal", () => {
  it("keeps every decimal and shows at least three", () => {
    expect(microToFieldDecimal(100_000_000)).toBe("100.000");
    expect(microToFieldDecimal(18_600_000)).toBe("18.600");
    expect(microToFieldDecimal(921_400)).toBe("0.9214");
    expect(microToFieldDecimal(12_345_678)).toBe("12.345678");
    expect(microToFieldDecimal(0)).toBe("0.000");
  });

  it("reads back to the same micro-units", () => {
    for (const micros of [
      0, 1, -1, 123_456, 921_400, 12_345_678, -12_345_678, 500_123_456, 999_999_999_999,
    ]) {
      expect(decimalToMicro(microToFieldDecimal(micros))).toBe(micros);
    }
  });
});
