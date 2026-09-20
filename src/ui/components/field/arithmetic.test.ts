import { describe, expect, it } from "vitest";
import { evaluateArithmetic } from "./arithmetic";

describe("evaluateArithmetic", () => {
  it("returns a plain number unchanged", () => {
    expect(evaluateArithmetic("120")).toBe(120);
    expect(evaluateArithmetic("1.2")).toBe(1.2);
    expect(evaluateArithmetic("0")).toBe(0);
  });

  it("evaluates the four operators", () => {
    expect(evaluateArithmetic("100*1.2")).toBe(120);
    expect(evaluateArithmetic("50+25")).toBe(75);
    expect(evaluateArithmetic("100-40")).toBe(60);
    expect(evaluateArithmetic("100/4")).toBe(25);
  });

  it("respects operator precedence", () => {
    expect(evaluateArithmetic("2+3*4")).toBe(14);
    expect(evaluateArithmetic("10-2*3")).toBe(4);
  });

  it("honours parentheses", () => {
    expect(evaluateArithmetic("(2+3)*4")).toBe(20);
    expect(evaluateArithmetic("(100+5)*1.2")).toBe(126);
    expect(evaluateArithmetic("((1+1))*5")).toBe(10);
  });

  it("handles unary minus and plus", () => {
    expect(evaluateArithmetic("-5")).toBe(-5);
    expect(evaluateArithmetic("10*-2")).toBe(-20);
    expect(evaluateArithmetic("+7")).toBe(7);
  });

  it("tolerates surrounding whitespace", () => {
    expect(evaluateArithmetic("  100 * 1.2  ")).toBe(120);
  });

  it("returns null for empty or whitespace-only input", () => {
    expect(evaluateArithmetic("")).toBeNull();
    expect(evaluateArithmetic("   ")).toBeNull();
  });

  it("returns null for incomplete or malformed expressions", () => {
    expect(evaluateArithmetic("100*")).toBeNull();
    expect(evaluateArithmetic("*5")).toBeNull();
    expect(evaluateArithmetic("(1+2")).toBeNull();
    expect(evaluateArithmetic("1+2)")).toBeNull();
    expect(evaluateArithmetic("1.2.3")).toBeNull();
    expect(evaluateArithmetic(".")).toBeNull();
    expect(evaluateArithmetic("1 2")).toBeNull();
  });

  it("returns null on division by zero", () => {
    expect(evaluateArithmetic("1/0")).toBeNull();
    expect(evaluateArithmetic("5/(2-2)")).toBeNull();
  });

  it("returns null for unknown characters (no eval, no identifiers)", () => {
    expect(evaluateArithmetic("100abc")).toBeNull();
    expect(evaluateArithmetic("alert(1)")).toBeNull();
    expect(evaluateArithmetic("2^3")).toBeNull();
  });
});
