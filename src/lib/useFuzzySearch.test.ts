import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useFuzzySearch } from "./useFuzzySearch";

type Item = { name: string };

const items: Item[] = [
  { name: "Apple" },
  { name: "Banana" },
  { name: "Apricot" },
  { name: "Blueberry" },
];

describe("useFuzzySearch", () => {
  it("returns empty array when query is shorter than 2 characters", () => {
    const { result } = renderHook(() => useFuzzySearch("a", items, ["name"]));
    expect(result.current).toEqual([]);
  });

  it("returns empty array when query is empty", () => {
    const { result } = renderHook(() => useFuzzySearch("", items, ["name"]));
    expect(result.current).toEqual([]);
  });

  it("returns matching items for a query of 2+ characters", () => {
    const { result } = renderHook(() => useFuzzySearch("ap", items, ["name"]));
    const names = result.current.map((i) => i.name);
    expect(names).toContain("Apple");
    expect(names).toContain("Apricot");
    expect(names).not.toContain("Banana");
  });

  it("returns empty array when no items match", () => {
    const { result } = renderHook(() => useFuzzySearch("zzz", items, ["name"]));
    expect(result.current).toEqual([]);
  });
});
