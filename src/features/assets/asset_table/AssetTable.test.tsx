import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Asset, AssetListings } from "@/bindings";
import { AssetTable } from "./AssetTable";

const navigateMock = vi.fn();
vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => navigateMock,
}));

// A stable `t`, as react-i18next gives: `useAssetTable`'s effect depends on it, and a new
// function each render would re-run it without end on an empty list.
const translation = vi.hoisted(() => ({
  t: (key: string) => key,
  i18n: { language: "en-US" },
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => translation,
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
let mockFetchError: { key: string } | null = null;
const archiveAssetMock = vi.fn();
const unarchiveAssetMock = vi.fn();
const fetchAssetsMock = vi.fn();
vi.mock("../useAssets", () => ({
  useAssets: () => ({
    assets: mockAssets,
    loading: false,
    fetchError: mockFetchError,
    archiveAsset: archiveAssetMock,
    unarchiveAsset: unarchiveAssetMock,
    fetchAssets: fetchAssetsMock,
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

// AST-015 (#446) — a first read that fails shows the error and Retry, never the empty state.
describe("AssetTable — a failed first read (AST-015)", () => {
  beforeEach(() => {
    mockAssets = [];
    mockOtherListings = [];
    fetchAssetsMock.mockClear();
  });
  afterEach(() => {
    mockFetchError = null;
  });

  it("shows the error and Retry instead of the empty state", () => {
    mockFetchError = { key: "error.DatabaseError" };
    render(<AssetTable searchTerm="" showArchived={false} />);
    expect(screen.getByText("asset.error_load")).toBeInTheDocument();
    expect(screen.queryByText("asset.empty")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "action.retry" }));
    expect(fetchAssetsMock).toHaveBeenCalledTimes(1);
  });
});

// AST-014 (#446) — a refused archive or unarchive keeps its dialog open and is told inside
// it; a success closes it.
describe("AssetTable — archive and unarchive refused (AST-014)", () => {
  beforeEach(() => {
    mockOtherListings = [];
    archiveAssetMock.mockReset();
    unarchiveAssetMock.mockReset();
  });

  it("keeps the archive dialog open with the refusal inside it", async () => {
    mockAssets = [makeAsset()];
    archiveAssetMock.mockResolvedValue({ error: { key: "error.DatabaseError" } });
    render(<AssetTable searchTerm="" showArchived={false} />);
    fireEvent.click(document.querySelector("#action-archive-asset-a1") as Element);
    fireEvent.click(document.querySelector("#confirm-archive-asset") as Element);
    await waitFor(() =>
      expect(document.querySelector("#confirm-archive-asset-error")?.textContent).toBe(
        "error.DatabaseError",
      ),
    );
    expect(document.querySelector("#confirm-archive-asset-dialog")).not.toBeNull();
  });

  it("keeps the unarchive dialog open with the refusal inside it", async () => {
    mockAssets = [makeAsset({ is_archived: true })];
    unarchiveAssetMock.mockResolvedValue({ error: { key: "error.DatabaseError" } });
    render(<AssetTable searchTerm="" showArchived />);
    fireEvent.click(document.querySelector("#action-unarchive-asset-a1") as Element);
    fireEvent.click(document.querySelector("#confirm-unarchive-asset") as Element);
    await waitFor(() =>
      expect(document.querySelector("#confirm-unarchive-asset-error")?.textContent).toBe(
        "error.DatabaseError",
      ),
    );
    expect(document.querySelector("#confirm-unarchive-asset-dialog")).not.toBeNull();
  });

  it("closes the archive dialog once the archive succeeds", async () => {
    mockAssets = [makeAsset()];
    archiveAssetMock.mockResolvedValue({ error: null });
    render(<AssetTable searchTerm="" showArchived={false} />);
    fireEvent.click(document.querySelector("#action-archive-asset-a1") as Element);
    fireEvent.click(document.querySelector("#confirm-archive-asset") as Element);
    await waitFor(() => expect(document.querySelector("#confirm-archive-asset-dialog")).toBeNull());
    expect(archiveAssetMock).toHaveBeenCalledWith("a1");
  });
});
