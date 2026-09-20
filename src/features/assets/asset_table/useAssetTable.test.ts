import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Asset } from "@/bindings";
import { useAssetTable } from "./useAssetTable";

const navigateMock = vi.fn();
vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => navigateMock,
}));

const makeAsset = (overrides: Partial<Asset> = {}): Asset => ({
  id: "a1",
  name: "Apple",
  reference: "AAPL",
  isin: null,
  class: "Stocks",
  currency: "USD",
  risk_level: 4,
  category: { id: "cat-1", name: "US Stocks" },
  is_archived: false,
  price_refresh_blocked: false,
  interest_bearing: false,
  ...overrides,
  exchange: null,
});

const activeAsset = makeAsset({
  id: "active",
  name: "Apple",
  reference: "AAPL",
  isin: null,
});
const archivedAsset = makeAsset({
  id: "archived",
  name: "Bond Fund",
  reference: "BND",
  isin: null,
  is_archived: true,
  exchange: null,
});

describe("useAssetTable", () => {
  beforeEach(() => {
    navigateMock.mockClear();
  });

  // F10 — navigation lives in the hook; openEditAsset drives the router-mounted modal
  it("openEditAsset navigates with modal=edit-asset and the asset id", () => {
    const { result } = renderHook(() => useAssetTable([activeAsset], "", false));
    act(() => result.current.openEditAsset("active"));
    expect(navigateMock).toHaveBeenCalledTimes(1);
    const arg = navigateMock.mock.calls[0]?.[0] as { search: (prev: object) => object };
    expect(arg.search({})).toEqual({ modal: "edit-asset", editAssetId: "active" });
  });

  // R7/R19 — filters out archived assets when showArchived is false
  it("filters out archived assets when showArchived is false", () => {
    const { result } = renderHook(() => useAssetTable([activeAsset, archivedAsset], "", false));
    expect(result.current.sortedAndFilteredAssets).toHaveLength(1);
    expect(result.current.sortedAndFilteredAssets[0]?.id).toBe("active");
  });

  // CSH-015 — system Cash Assets are filtered out of the Asset Manager regardless of showArchived
  it("filters out system Cash Assets even when showArchived is true (CSH-015)", () => {
    const cashAsset = makeAsset({
      id: "system-cash-eur",
      name: "Cash EUR",
      reference: "EUR",
      isin: null,
      class: "Cash",
    });
    const { result } = renderHook(() =>
      useAssetTable([activeAsset, cashAsset, archivedAsset], "", true),
    );
    expect(result.current.sortedAndFilteredAssets.map((a) => a.id)).not.toContain(
      "system-cash-eur",
    );
  });

  // R19 — includes archived assets when showArchived is true
  it("includes archived assets when showArchived is true", () => {
    const { result } = renderHook(() => useAssetTable([activeAsset, archivedAsset], "", true));
    expect(result.current.sortedAndFilteredAssets).toHaveLength(2);
  });

  // R16 — fuzzy search applies on currently displayed assets only
  it("fuzzy search applies only to displayed assets (showArchived=false)", () => {
    const { result } = renderHook(() => useAssetTable([activeAsset, archivedAsset], "Bond", false));
    // "Bond Fund" is archived, not displayed when showArchived=false
    expect(result.current.sortedAndFilteredAssets).toHaveLength(0);
  });

  it("fuzzy search finds archived asset when showArchived=true", () => {
    const { result } = renderHook(() => useAssetTable([activeAsset, archivedAsset], "Bond", true));
    expect(result.current.sortedAndFilteredAssets).toHaveLength(1);
    expect(result.current.sortedAndFilteredAssets[0]?.id).toBe("archived");
  });

  // R7/R17 — sorted by name ascending by default
  it("sorts by name ascending by default", () => {
    const assets = [
      makeAsset({ id: "z", name: "Zoom", reference: "ZM" }),
      makeAsset({ id: "a", name: "Apple", reference: "AAPL" }),
    ];
    const { result } = renderHook(() => useAssetTable(assets, "", false));
    expect(result.current.sortedAndFilteredAssets[0]?.name).toBe("Apple");
    expect(result.current.sortedAndFilteredAssets[1]?.name).toBe("Zoom");
  });

  // R17 — toggles sort direction on second click of same column
  it("toggles sort direction on second click of same column", () => {
    const assets = [
      makeAsset({ id: "z", name: "Zoom", reference: "ZM" }),
      makeAsset({ id: "a", name: "Apple", reference: "AAPL" }),
    ];
    const { result } = renderHook(() => useAssetTable(assets, "", false));

    // first click: already asc by name (default), second click → desc
    act(() => result.current.handleSort("name"));
    expect(result.current.sortConfig.direction).toBe("desc");
    expect(result.current.sortedAndFilteredAssets[0]?.name).toBe("Zoom");
  });

  // AST-017 — every primary sort breaks ties by name ascending, independent of
  // the primary direction.
  it("breaks ties by name ascending on a primary sort, independent of direction", () => {
    const assets = [
      makeAsset({ id: "z", name: "Zoom", reference: "ZM", currency: "USD" }),
      makeAsset({ id: "a", name: "Apple", reference: "AAPL", currency: "USD" }),
      makeAsset({ id: "m", name: "Mango", reference: "MNG", currency: "EUR" }),
    ];
    const { result } = renderHook(() => useAssetTable(assets, "", false));

    // Primary sort by currency ascending → EUR group, then USD group with names A→Z.
    act(() => result.current.handleSort("currency"));
    expect(result.current.sortConfig).toMatchObject({ key: "currency", direction: "asc" });
    expect(result.current.sortedAndFilteredAssets.map((a) => a.name)).toEqual([
      "Mango",
      "Apple",
      "Zoom",
    ]);

    // Toggle to descending → USD group first, but names still A→Z within the tie.
    act(() => result.current.handleSort("currency"));
    expect(result.current.sortConfig).toMatchObject({ key: "currency", direction: "desc" });
    expect(result.current.sortedAndFilteredAssets.map((a) => a.name)).toEqual([
      "Apple",
      "Zoom",
      "Mango",
    ]);
  });
});
