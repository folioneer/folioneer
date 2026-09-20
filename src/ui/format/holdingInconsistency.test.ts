import { describe, expect, it } from "vitest";
import type { HoldingInconsistency } from "@/bindings";
import { formatHoldingInconsistency } from "./holdingInconsistency";

// SYN-040 / CFR-042 — shared by the holding row and the sync status list; micro-unit
// fields are formatted, not raw.
describe("formatHoldingInconsistency", () => {
  it("maps Oversold with the formatted oversold quantity", () => {
    const reason: HoldingInconsistency = { Oversold: { quantity: -5_000_000 } };
    expect(formatHoldingInconsistency(reason)).toEqual({
      key: "sync.inconsistency.oversold",
      vars: { quantity: "-5" },
    });
  });

  it("maps CashOverdrawn with the formatted overdrawn amount", () => {
    const reason: HoldingInconsistency = { CashOverdrawn: { amount: -125_500_000 } };
    expect(formatHoldingInconsistency(reason)).toEqual({
      key: "sync.inconsistency.cash_overdrawn",
      vars: { amount: "-125,50" },
    });
  });
});
