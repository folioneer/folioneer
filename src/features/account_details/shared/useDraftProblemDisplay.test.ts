import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { TransactionDraftError } from "@/bindings";
import { draftProblemField, useDraftProblemDisplay } from "./useDraftProblemDisplay";

const QUANTITY: TransactionDraftError = { code: "QuantityNotPositive" };
const MESSAGE = { key: "error.QuantityNotPositive" };

describe("useDraftProblemDisplay", () => {
  // TRX-067 — a problem on a field not typed in yet is a hint saying what to enter; once
  // typed in, it is that field's error.
  it("says what to enter, then shows the field's error once typed in", () => {
    const { result } = renderHook(() => useDraftProblemDisplay(QUANTITY, MESSAGE));
    expect(result.current.hint).toEqual({ key: "transaction.hint_enter_quantity" });
    expect(result.current.fieldErrors).toEqual({});
    expect(result.current.alert).toBeNull();

    act(() => result.current.touch("quantity"));
    expect(result.current.hint).toBeNull();
    expect(result.current.fieldErrors).toEqual({ quantity: MESSAGE });
  });

  // TRX-067 — a check that could not run is an error beside the actions; a clean draft
  // shows nothing; an account or an asset not chosen yet is a hint, like any field.
  it("shows a failed check as an alert, and nothing for a clean draft", () => {
    const failed = renderHook(() => useDraftProblemDisplay(null, { key: "error.Unknown" }));
    expect(failed.result.current.alert).toEqual({ key: "error.Unknown" });

    const clean = renderHook(() => useDraftProblemDisplay(null, null));
    expect(clean.result.current).toMatchObject({ fieldErrors: {}, hint: null, alert: null });

    const missing = renderHook(() =>
      useDraftProblemDisplay(
        { code: "AccountMissing" },
        { key: "transaction.error_validation_account" },
      ),
    );
    expect(missing.result.current.hint).toEqual({ key: "transaction.hint_select_account" });
    expect(missing.result.current.alert).toBeNull();
  });

  // Each problem is shown on the field the user fixes it in: a total that is not positive is
  // the unit price's while the price is typed, the total's while the total is.
  it("puts each problem on the field that fixes it", () => {
    expect(draftProblemField({ code: "Oversell", available: 5, requested: 8 }, "price")).toBe(
      "quantity",
    );
    expect(draftProblemField({ code: "TotalAmountBelowFees" }, "total")).toBe("total");
    expect(draftProblemField({ code: "TotalAmountNotPositive" }, "price")).toBe("unitPrice");
    expect(draftProblemField({ code: "TotalAmountNotPositive" }, "total")).toBe("total");
    expect(draftProblemField({ code: "TotalCostMissing" }, "price")).toBe("totalCost");
    expect(draftProblemField({ code: "DateInFuture" }, "price")).toBe("date");
    expect(draftProblemField({ code: "AssetMissing" }, "price")).toBe("asset");
    expect(draftProblemField({ code: "TradeOnCashAsset" }, "price")).toBeNull();
    expect(draftProblemField(null, "price")).toBeNull();
  });
});
