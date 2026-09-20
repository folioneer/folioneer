import { renderHook } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Asset, CreateAssetDTO, UpdateAssetDTO } from "@/bindings";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";

const {
  mockCreateAsset,
  mockUpdateAsset,
  mockArchiveAsset,
  mockUnarchiveAsset,
  mockDeleteAsset,
  mockFetchAssets,
  mockShowSnackbar,
} = vi.hoisted(() => ({
  mockCreateAsset: vi.fn(),
  mockUpdateAsset: vi.fn(),
  mockArchiveAsset: vi.fn(),
  mockUnarchiveAsset: vi.fn(),
  mockDeleteAsset: vi.fn(),
  mockFetchAssets: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("./gateway", () => ({
  assetGateway: {
    createAsset: mockCreateAsset,
    updateAsset: mockUpdateAsset,
    archiveAsset: mockArchiveAsset,
    unarchiveAsset: mockUnarchiveAsset,
    deleteAsset: mockDeleteAsset,
  },
}));

vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn() },
}));

const { useAssets } = await import("./useAssets");

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

describe("useAssets", () => {
  beforeEach(() => {
    mockCreateAsset.mockReset();
    mockUpdateAsset.mockReset();
    mockArchiveAsset.mockReset();
    mockUnarchiveAsset.mockReset();
    mockDeleteAsset.mockReset();
    mockFetchAssets.mockReset();
    mockShowSnackbar.mockReset();
    // Override the store's fetchAssets with the mock so we can spy on calls
    // without exercising the real gateway path.
    useAppStore.setState({
      assets: [
        { id: "a1", name: "Apple", is_archived: false, exchange: null },
        { id: "a2", name: "OldCo", is_archived: true, exchange: null },
      ] as Asset[],
      isLoadingAssets: false,
      assetsError: null,
      fetchAssets: mockFetchAssets,
    });
  });

  // ── activeCount ───────────────────────────────────────────────────────────────

  it("activeCount excludes archived assets", () => {
    const { result } = renderHook(() => useAssets());
    expect(result.current.activeCount).toBe(1);
  });

  // ── addAsset ──────────────────────────────────────────────────────────────────

  it("addAsset calls gateway, fetches assets, and shows snackbar on success", async () => {
    const asset = makeAsset();
    mockCreateAsset.mockResolvedValue({ status: "ok", data: asset });
    const { result } = renderHook(() => useAssets());
    let ret: { data: Asset | null; error: I18nMessage | null } = {
      data: null,
      error: null,
    };
    const dto: CreateAssetDTO = {
      name: "Apple",
      class: "Stocks",
      category_id: "cat-1",
      currency: "USD",
      risk_level: 4,
      reference: "AAPL",
      isin: null,
      exchange: null,
      interest_bearing: false,
    };
    await act(async () => {
      ret = await result.current.addAsset(dto);
    });
    expect(mockCreateAsset).toHaveBeenCalledWith(dto);
    expect(mockFetchAssets).toHaveBeenCalled();
    expect(mockShowSnackbar).toHaveBeenCalledWith("asset.success_created", "success");
    expect(ret.data).toEqual(asset);
    expect(ret.error).toBeNull();
  });

  it("addAsset returns error code on failure without fetching", async () => {
    mockCreateAsset.mockResolvedValue({
      status: "error",
      error: { code: "NameAlreadyExists" },
    });
    const { result } = renderHook(() => useAssets());
    let ret: { data: Asset | null; error: I18nMessage | null } = {
      data: null,
      error: null,
    };
    const dto: CreateAssetDTO = {
      name: "Apple",
      class: "Stocks",
      category_id: "cat-1",
      currency: "USD",
      risk_level: 4,
      reference: "AAPL",
      isin: null,
      exchange: null,
      interest_bearing: false,
    };
    await act(async () => {
      ret = await result.current.addAsset(dto);
    });
    expect(mockFetchAssets).not.toHaveBeenCalled();
    expect(ret.error).toEqual({ key: "error.NameAlreadyExists" });
  });

  // ── updateAsset ───────────────────────────────────────────────────────────────

  it("updateAsset calls gateway and shows snackbar on success", async () => {
    const asset = makeAsset({ name: "Apple Inc." });
    mockUpdateAsset.mockResolvedValue({ status: "ok", data: asset });
    const { result } = renderHook(() => useAssets());
    const dto: UpdateAssetDTO = {
      asset_id: "a1",
      name: "Apple Inc.",
      class: "Stocks",
      category_id: "cat-1",
      currency: "USD",
      risk_level: 4,
      reference: "AAPL",
      isin: null,
      exchange: null,
      interest_bearing: false,
    };
    let ret: { data: Asset | null; error: I18nMessage | null } = {
      data: null,
      error: { key: "sentinel" },
    };
    await act(async () => {
      ret = await result.current.updateAsset(dto);
    });
    expect(mockUpdateAsset).toHaveBeenCalledWith(dto);
    expect(mockShowSnackbar).toHaveBeenCalledWith("asset.success_updated", "success");
    expect(ret.data).toEqual(asset);
    expect(ret.error).toBeNull();
  });

  it("updateAsset returns error code on failure", async () => {
    mockUpdateAsset.mockResolvedValue({
      status: "error",
      error: { code: "NameAlreadyExists" },
    });
    const { result } = renderHook(() => useAssets());
    const dto: UpdateAssetDTO = {
      asset_id: "a1",
      name: "Apple Inc.",
      class: "Stocks",
      category_id: "cat-1",
      currency: "USD",
      risk_level: 4,
      reference: "AAPL",
      isin: null,
      exchange: null,
      interest_bearing: false,
    };
    let ret: { data: Asset | null; error: I18nMessage | null } = {
      data: null,
      error: null,
    };
    await act(async () => {
      ret = await result.current.updateAsset(dto);
    });
    expect(ret.error).toEqual({ key: "error.NameAlreadyExists" });
  });

  // ── archiveAsset ──────────────────────────────────────────────────────────────

  it("archiveAsset calls gateway and shows snackbar on success", async () => {
    mockArchiveAsset.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useAssets());
    let ret: { error: I18nMessage | null } = { error: null };
    await act(async () => {
      ret = await result.current.archiveAsset("a1");
    });
    expect(mockArchiveAsset).toHaveBeenCalledWith("a1");
    expect(mockShowSnackbar).toHaveBeenCalledWith("asset.success_archived", "success");
    expect(ret.error).toBeNull();
  });

  it("archiveAsset returns error code on failure", async () => {
    mockArchiveAsset.mockResolvedValue({
      status: "error",
      error: { code: "ActiveHoldings" },
    });
    const { result } = renderHook(() => useAssets());
    let ret: { error: I18nMessage | null } = { error: null };
    await act(async () => {
      ret = await result.current.archiveAsset("a1");
    });
    expect(ret.error).toEqual({ key: "error.ActiveHoldings" });
  });

  // ── unarchiveAsset ────────────────────────────────────────────────────────────

  it("unarchiveAsset calls gateway and shows snackbar on success", async () => {
    mockUnarchiveAsset.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useAssets());
    let ret: { error: I18nMessage | null } = { error: { key: "sentinel" } };
    await act(async () => {
      ret = await result.current.unarchiveAsset("a2");
    });
    expect(mockUnarchiveAsset).toHaveBeenCalledWith("a2");
    expect(mockShowSnackbar).toHaveBeenCalledWith("asset.success_unarchived", "success");
    expect(ret.error).toBeNull();
  });

  // ── deleteAsset ───────────────────────────────────────────────────────────────

  it("deleteAsset calls gateway and shows info snackbar on success", async () => {
    mockDeleteAsset.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useAssets());
    let ret: { error: I18nMessage | null } = { error: null };
    await act(async () => {
      ret = await result.current.deleteAsset("a1");
    });
    expect(mockDeleteAsset).toHaveBeenCalledWith("a1");
    expect(mockShowSnackbar).toHaveBeenCalledWith("asset.success_deleted", "info");
    expect(ret.error).toBeNull();
  });

  it("deleteAsset returns error code on failure", async () => {
    mockDeleteAsset.mockResolvedValue({
      status: "error",
      error: { code: "ExistingTransactions" },
    });
    const { result } = renderHook(() => useAssets());
    let ret: { error: I18nMessage | null } = { error: null };
    await act(async () => {
      ret = await result.current.deleteAsset("a1");
    });
    expect(ret.error).toEqual({ key: "error.ExistingTransactions" });
  });
});
