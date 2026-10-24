import { describe, expect, it } from "vitest";
import { microToFormattedQuantity } from "@/lib/microUnits";
import { takeOutRefusal } from "./presenter";

// SEL-046 / WOF-055 — a refusal for lack of units names what was held of the asset on the
// date; any other refusal is not one of them.
describe("takeOutRefusal", () => {
  it("names what was held on the date above which the quantity was refused", () => {
    expect(
      takeOutRefusal({ code: "Oversell", available: 200_000 }, "BTC", "2026-10-21", "fr"),
    ).toEqual({
      key: "transaction.error_over_held_on",
      vars: { held: microToFormattedQuantity(200_000), reference: "BTC", date: "21/10/2026" },
    });
  });

  it("names the date on which nothing was held", () => {
    expect(takeOutRefusal({ code: "ClosedPosition" }, "BTC", "2026-10-21", "en")).toEqual({
      key: "transaction.error_none_held_on",
      vars: { reference: "BTC", date: "10/21/2026" },
    });
  });

  it("says a later transaction would be left short", () => {
    expect(takeOutRefusal({ code: "CascadingOversell" }, "BTC", "2026-10-21", "en")).toEqual({
      key: "transaction.error_later_left_short",
    });
  });

  it("is null for any other refusal", () => {
    expect(takeOutRefusal({ code: "DateInFuture" }, "BTC", "2026-10-21", "en")).toBeNull();
  });
});
