import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useCategoryTable } from "./useCategoryTable";

const cats = [
  { id: "2", name: "Bonds" },
  { id: "1", name: "Actions" },
  { id: "3", name: "Immobilier" },
];

describe("useCategoryTable", () => {
  it("defaults to name asc sort", () => {
    const { result } = renderHook(() => useCategoryTable(cats, ""));
    expect(result.current.sortConfig).toEqual({
      key: "name",
      direction: "asc",
    });
    expect(result.current.sortedAndFilteredCategories.map((c) => c.name)).toEqual([
      "Actions",
      "Bonds",
      "Immobilier",
    ]);
  });

  it("toggles to desc on second click of the same column", () => {
    const { result } = renderHook(() => useCategoryTable(cats, ""));
    act(() => result.current.handleSort("name"));
    expect(result.current.sortConfig.direction).toBe("desc");
    expect(result.current.sortedAndFilteredCategories.at(0)?.name).toBe("Immobilier");
  });

  it("resets to asc when switching to a new column", () => {
    const { result } = renderHook(() => useCategoryTable(cats, ""));
    act(() => result.current.handleSort("name")); // now desc
    act(() => result.current.handleSort("id")); // new column → asc
    expect(result.current.sortConfig).toEqual({ key: "id", direction: "asc" });
  });

  it("filters by search term (case-insensitive)", () => {
    const { result } = renderHook(() => useCategoryTable(cats, "bo"));
    expect(result.current.sortedAndFilteredCategories).toHaveLength(1);
    expect(result.current.sortedAndFilteredCategories.at(0)?.name).toBe("Bonds");
  });

  it("returns empty list when no category matches the search", () => {
    const { result } = renderHook(() => useCategoryTable(cats, "xyz"));
    expect(result.current.sortedAndFilteredCategories).toHaveLength(0);
  });
});
