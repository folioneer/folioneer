import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Asset } from "@/bindings";
import { AssetTable } from "./AssetTable";

const navigateMock = vi.fn();
vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => navigateMock,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en-US" } }),
}));

vi.mock("@/lib/logger", () => ({
  logger: { info: vi.fn(), error: vi.fn() },
}));

let mockAssets: Asset[] = [];
vi.mock("../useAssets", () => ({
  useAssets: () => ({
    assets: mockAssets,
    loading: false,
    fetchError: null,
    archiveAsset: vi.fn(),
    unarchiveAsset: vi.fn(),
    fetchAssets: vi.fn(),
  }),
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
  exchange: null,
  ...overrides,
});

const expectEditNavigation = (assetId: string) => {
  expect(navigateMock).toHaveBeenCalledTimes(1);
  const arg = navigateMock.mock.calls[0]?.[0] as { search: (prev: object) => object };
  expect(arg.search({})).toEqual({ modal: "edit-asset", editAssetId: assetId });
};

describe("AssetTable — router-driven edit", () => {
  beforeEach(() => {
    navigateMock.mockClear();
    mockAssets = [makeAsset()];
  });

  it("edit button opens the edit-asset modal via URL params", () => {
    render(<AssetTable searchTerm="" showArchived={false} />);
    fireEvent.click(screen.getByRole("button", { name: "asset.action_edit" }));
    expectEditNavigation("a1");
  });

  it("double-clicking a row opens the edit-asset modal via URL params", () => {
    render(<AssetTable searchTerm="" showArchived={false} />);
    const row = screen.getByText("Apple").closest("tr");
    if (!row) throw new Error("expected an asset row");
    fireEvent.doubleClick(row);
    expectEditNavigation("a1");
  });

  it("does not open edit on double-click for an archived row", () => {
    mockAssets = [makeAsset({ is_archived: true })];
    render(<AssetTable searchTerm="" showArchived={true} />);
    const row = screen.getByText("Apple").closest("tr");
    if (!row) throw new Error("expected an asset row");
    fireEvent.doubleClick(row);
    expect(navigateMock).not.toHaveBeenCalled();
  });

  it("pressing Enter on a row opens the edit-asset modal via URL params", () => {
    render(<AssetTable searchTerm="" showArchived={false} />);
    const row = screen.getByText("Apple").closest("tr");
    if (!row) throw new Error("expected an asset row");
    fireEvent.keyDown(row, { key: "Enter" });
    expectEditNavigation("a1");
  });

  it("Enter bubbling from the row's edit button does not double-open the modal", () => {
    render(<AssetTable searchTerm="" showArchived={false} />);
    fireEvent.keyDown(screen.getByRole("button", { name: "asset.action_edit" }), { key: "Enter" });
    expect(navigateMock).not.toHaveBeenCalled();
  });
});
