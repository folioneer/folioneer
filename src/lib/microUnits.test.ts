import { afterEach, describe, expect, it } from "vitest";
import {
  addQuantities,
  compareQuantities,
  decimalsOf,
  decimalToDisplayed,
  decimalToMicro,
  decimalToNumber,
  decimalToQuantity,
  formatQuantity,
  isPositiveQuantity,
  MONEY_DECIMALS,
  microPerUnit,
  microToExactDecimal,
  microToFieldDecimal,
  quantityCost,
  quantityToFieldDecimal,
  setDisplayLocale,
  subtractQuantities,
  typedToDecimal,
} from "./microUnits";

describe("decimalToMicro", () => {
  // TRX-024 — a decimal written with a dot becomes micro-units; a comma is no number
  // here: what is typed is read by `typedToDecimal` first (NUM-010).
  it("reads a decimal written with a dot", () => {
    expect(decimalToMicro("1.5")).toBe(1_500_000);
    expect(decimalToMicro("1,5")).toBe(0);
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

// TRX-024 / DEBT-073 — a field filled from a figure reads back to that figure: every decimal
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

// NUM-010/011 — one conversion reads what is typed, one writes what is shown, each in the
// two display languages.
describe("typedToDecimal and decimalToDisplayed", () => {
  afterEach(() => setDisplayLocale("fr"));

  it("in French, read a comma or a dot and write a comma", () => {
    setDisplayLocale("fr");
    expect(typedToDecimal("1,5")).toBe("1.5");
    expect(typedToDecimal("1.5")).toBe("1.5");
    expect(typedToDecimal("100*1,2")).toBe("100*1.2");
    // A thousands separator is no part of a number: both together are not one.
    expect(decimalToNumber(typedToDecimal("1.234,56"))).toBeNaN();
    expect(decimalToNumber(typedToDecimal("1 234,5"))).toBeNaN();
    expect(decimalToDisplayed("12.345678")).toBe("12,345678");
    expect(decimalToMicro(typedToDecimal(decimalToDisplayed("12.345678")))).toBe(12_345_678);
  });

  it("in English, read the dot only and write the dot", () => {
    setDisplayLocale("en");
    expect(typedToDecimal("1.5")).toBe("1.5");
    expect(typedToDecimal("1,5")).toBe("1,5");
    expect(decimalToMicro(typedToDecimal("1,5"))).toBe(0);
    expect(decimalToDisplayed("12.345678")).toBe("12.345678");
  });

  it("follow a regional variant of the language", () => {
    setDisplayLocale("fr-CH");
    expect(decimalToDisplayed("1.5")).toBe("1,5");
    setDisplayLocale("en-GB");
    expect(decimalToDisplayed("1.5")).toBe("1.5");
  });
});

// A text that only starts like a number is not one.
describe("decimalToNumber", () => {
  it("reads a decimal written with a dot, and nothing else", () => {
    expect(decimalToNumber("1.5")).toBe(1.5);
    expect(decimalToNumber(" 12 ")).toBe(12);
    expect(decimalToNumber("1,5")).toBeNaN();
    expect(decimalToNumber("12abc")).toBeNaN();
    expect(decimalToNumber("")).toBeNaN();
  });
});

describe("a quantity as decimal text (TRX-024B, TRX-024H, TRX-024I)", () => {
  afterEach(() => setDisplayLocale("en"));

  it("TRX-024H: keeps eight decimals, drops a ninth and trailing zeros", () => {
    expect(decimalToQuantity("0.00000001")).toBe("0.00000001");
    expect(decimalToQuantity("0.000000019")).toBe("0.00000001");
    expect(decimalToQuantity("1.50")).toBe("1.5");
    expect(decimalToQuantity("007")).toBe("7");
    expect(decimalToQuantity("-0.5")).toBe("-0.5");
    expect(decimalToQuantity("abc")).toBe("0");
    expect(decimalToQuantity("")).toBe("0");
    for (const notAFigure of ["1e3", "0x10", "Infinity", "."]) {
      expect(decimalToQuantity(notAFigure)).toBe("0");
      expect(isPositiveQuantity(notAFigure)).toBe(false);
    }
  });

  it("TRX-024H: keeps six decimals for money", () => {
    expect(decimalToQuantity("1.1234567", MONEY_DECIMALS)).toBe("1.123456");
    expect(decimalsOf("system-cash-EUR")).toBe(MONEY_DECIMALS);
    expect(decimalsOf("asset-btc")).toBe(8);
  });

  it("sums and compares exactly, past what a number holds", () => {
    expect(addQuantities("0.1", "0.2")).toBe("0.3");
    expect(addQuantities("90000000.00000001", "0.00000001")).toBe("90000000.00000002");
    expect(subtractQuantities("0.3", "0.00000001")).toBe("0.29999999");
    expect(subtractQuantities("0.1", "0.2")).toBe("-0.1");
    expect(compareQuantities("90000000.00000002", "90000000.00000001")).toBe(1);
    expect(compareQuantities("0.00000001", "0.00000002")).toBe(-1);
    expect(compareQuantities("1.50", "1.5")).toBe(0);
  });

  it("TRX-024J: sees a satoshi as a quantity, a ninth decimal as none", () => {
    expect(isPositiveQuantity("0.00000001")).toBe(true);
    expect(isPositiveQuantity("0.000000001")).toBe(false);
    expect(isPositiveQuantity("0")).toBe(false);
  });

  it("costs and prices a quantity, rounded down, zero for none", () => {
    expect(quantityCost("0.5", 20_000_000_000)).toBe(10_000_000_000);
    expect(quantityCost("0.00000001", 50_000_000_000)).toBe(500);
    expect(microPerUnit(500, "0.00000001")).toBe(50_000_000_000);
    expect(microPerUnit(1_000_000, "0")).toBe(0);
  });

  it("writes a form field with every decimal and at least three", () => {
    expect(quantityToFieldDecimal("100")).toBe("100.000");
    expect(quantityToFieldDecimal("1.5")).toBe("1.500");
    expect(quantityToFieldDecimal("0.00000001")).toBe("0.00000001");
  });

  it("TRX-024I: shows every decimal in the display language, past what a number holds", () => {
    setDisplayLocale("en");
    expect(formatQuantity("2")).toBe("2");
    expect(formatQuantity("0.00000001")).toBe("0.00000001");
    expect(formatQuantity("0.00000001", 3)).toBe("0.00000001");
    expect(formatQuantity("1.5", 3)).toBe("1.500");
    expect(formatQuantity("90000000.00000001")).toBe("90,000,000.00000001");
    setDisplayLocale("fr");
    expect(formatQuantity("0.00000001")).toBe("0,00000001");
  });
});
