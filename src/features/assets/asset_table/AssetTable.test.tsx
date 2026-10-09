import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Asset, AssetListings } from "@/bindings";
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

let mockOtherListings: AssetListings[] = [];
vi.mock("../gateway", () => ({
  assetGateway: {
    getOtherListings: async () => ({ status: "ok", data: mockOtherListings }),
  },
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
  kind: "Custom",
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
    mockOtherListings = [];
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

describe("AssetTable — kinds", () => {
  beforeEach(() => {
    navigateMock.mockClear();
    mockOtherListings = [];
  });

  // CSH-015 — the application's cash shows as such and offers no action, by click or key.
  it("shows a cash row as managed by the application, with no action", () => {
    mockAssets = [
      makeAsset({ id: "system-cash-eur", name: "Cash EUR", kind: "Cash", class: "Cash" }),
    ];
    render(<AssetTable searchTerm="" showArchived={false} />);

    expect(document.getElementById("asset-managed-system-cash-eur")).not.toBeNull();
    expect(document.getElementById("action-edit-asset-system-cash-eur")).toBeNull();
    expect(document.getElementById("action-archive-asset-system-cash-eur")).toBeNull();

    const row = document.getElementById("asset-row-system-cash-eur") as HTMLElement;
    fireEvent.doubleClick(row);
    fireEvent.keyDown(row, { key: "Enter" });
    expect(navigateMock).not.toHaveBeenCalled();
  });

  // AST-039 — an asset whose instrument has other listings says so under its name; an asset
  // alone says nothing.
  it("says under the name what else an instrument is held as", async () => {
    mockAssets = [makeAsset({ id: "ams", kind: "Listed" }), makeAsset({ id: "alone" })];
    mockOtherListings = [
      {
        asset_id: "ams",
        others: [
          {
            asset_id: "nas",
            reference: "ASML",
            exchange: { code: "XNAS", label: "Nasdaq" },
            currency: "USD",
          },
          { asset_id: "otc", reference: "ASMLF", exchange: null, currency: "USD" },
        ],
      },
    ];
    render(<AssetTable searchTerm="" showArchived={false} />);

    await waitFor(() => expect(document.getElementById("asset-also-held-ams")).not.toBeNull());
    const line = document.getElementById("asset-also-held-ams");
    expect(line?.getAttribute("title")).toBe("ASML · Nasdaq · USD\nASMLF · USD");
    expect(document.getElementById("asset-also-held-alone")).toBeNull();
  });
});
