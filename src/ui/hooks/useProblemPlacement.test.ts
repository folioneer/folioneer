import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useProblemPlacement } from "./useProblemPlacement";

type Field = "quantity" | "price";

describe("useProblemPlacement", () => {
  // A problem on a field not typed in yet names the field; once typed in, it is its error.
  it("names an untouched field, then shows its error once typed in", () => {
    const { result } = renderHook(() => useProblemPlacement<Field, string>("quantity", "too low"));
    expect(result.current).toMatchObject({
      untouchedField: "quantity",
      fieldErrors: {},
      alert: null,
    });

    act(() => result.current.touch("price"));
    expect(result.current.untouchedField).toBe("quantity");

    act(() => result.current.touch("quantity"));
    expect(result.current).toMatchObject({
      untouchedField: null,
      fieldErrors: { quantity: "too low" },
      alert: null,
    });
  });

  // A problem that concerns no field is shown beside the actions.
  it("shows a problem without a field as an alert", () => {
    const { result } = renderHook(() => useProblemPlacement<Field, string>(null, "cannot check"));
    expect(result.current).toMatchObject({
      alert: "cannot check",
      untouchedField: null,
      fieldErrors: {},
    });
  });

  // No problem, nothing shown — whatever was typed.
  it("shows nothing without a problem", () => {
    const { result } = renderHook(() => useProblemPlacement<Field, string>("quantity", null));
    act(() => result.current.touch("quantity"));
    expect(result.current).toMatchObject({ alert: null, untouchedField: null, fieldErrors: {} });
  });
});
