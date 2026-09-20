import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useComboboxField } from "./useComboboxField";

interface Item {
  id: string;
  name: string;
}

const items: Item[] = [
  { id: "1", name: "Apple" },
  { id: "2", name: "Banana" },
  { id: "3", name: "Cherry" },
];

describe("useComboboxField", () => {
  it("offers the full list when the query is empty", () => {
    const { result } = renderHook(() => useComboboxField(items, "name"));
    expect(result.current.filteredItems).toHaveLength(3);
  });

  it("still offers the full list at exactly 1 character (below the fuzzy threshold)", () => {
    const { result } = renderHook(() => useComboboxField(items, "name"));
    act(() => result.current.setQuery("B"));
    expect(result.current.filteredItems).toHaveLength(3);
  });

  it("fuzzy-filters once the query reaches 2 characters", () => {
    const { result } = renderHook(() => useComboboxField(items, "name"));
    act(() => result.current.setQuery("Ban"));
    expect(result.current.filteredItems.map((item) => item.name)).toEqual(["Banana"]);
  });
});
