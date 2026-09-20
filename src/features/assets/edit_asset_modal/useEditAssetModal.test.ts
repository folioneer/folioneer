import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Asset, Exchange } from "@/bindings";
import { useEditAssetModal } from "./useEditAssetModal";

const mockUpdateAsset = vi.fn();

const mockAsset: Asset = {
  id: "asset-1",
  name: "Apple Inc.",
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
};

vi.mock("../useAssets", () => ({
  useAssets: () => ({
    updateAsset: mockUpdateAsset,
    assets: [mockAsset],
    activeCount: 1,
    loading: false,
    fetchError: null,
    fetchAssets: vi.fn(),
    addAsset: vi.fn(),
    archiveAsset: vi.fn(),
    unarchiveAsset: vi.fn(),
    deleteAsset: vi.fn(),
  }),
}));

vi.mock("@/features/categories/useCategories", () => ({
  useCategories: () => ({
    categories: [{ id: "cat-1", name: "US Stocks", is_system: false }],
    loading: false,
  }),
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn() },
}));

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

describe("useEditAssetModal", () => {
  beforeEach(() => {
    mockUpdateAsset.mockReset();
  });

  // R12 — class change in edit mode does NOT auto-fill risk_level
  it("does not auto-fill risk_level when class changes in edit mode", () => {
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: mockAsset, onClose }));

    act(() => {
      result.current.handleClassChange("Bonds");
    });

    // risk_level should remain 4 (from mockAsset), not 2 (Bonds default)
    expect(result.current.formData.risk_level).toBe(4);
  });

  // R9 — duplicate warning excludes self
  it("does not warn about duplicate reference for own asset", () => {
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: mockAsset, onClose }));

    // reference is already "AAPL" (same as mockAsset) — should not warn since it's self
    expect(result.current.duplicateWarning).toBe(false);
  });

  // R14 — does not close on backend error, exposes error message
  it("does not close and exposes error on backend failure", async () => {
    mockUpdateAsset.mockResolvedValue({ data: null, error: "Archived asset" });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: mockAsset, onClose }));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toBe("Archived asset");
    expect(onClose).not.toHaveBeenCalled();
  });

  // R14 — closes on success
  it("calls onClose on successful update", async () => {
    mockUpdateAsset.mockResolvedValue({ data: mockAsset, error: null });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: mockAsset, onClose }));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toBeNull();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  // AST-012 — editing an asset with an existing exchange pre-fills the picker
  it("pre-fills exchange from asset when asset has an exchange (AST-012)", () => {
    const exchange: Exchange = { code: "XPAR", label: "Euronext Paris" };
    const assetWithExchange: Asset = { ...mockAsset, exchange };
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: assetWithExchange, onClose }));
    expect(result.current.formData.exchange).toEqual(exchange);
  });

  // AST-012 — asset with no exchange initialises picker to null
  it("initialises exchange to null when asset has no exchange", () => {
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: mockAsset, onClose }));
    expect(result.current.formData.exchange).toBeNull();
  });

  // AST-022 — clearing the picker submits exchange: null
  it("submits exchange: null when picker is cleared (AST-022 — clear)", async () => {
    const exchange: Exchange = { code: "XPAR", label: "Euronext Paris" };
    const assetWithExchange: Asset = { ...mockAsset, exchange };
    mockUpdateAsset.mockResolvedValue({ data: assetWithExchange, error: null });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: assetWithExchange, onClose }));

    act(() => {
      result.current.handleExchangeChange(null);
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpdateAsset).toHaveBeenCalledWith(expect.objectContaining({ exchange: null }));
  });

  // AST-023 — editing an asset with an existing ISIN pre-fills the form field
  it("pre-fills isin from asset.isin", () => {
    const assetWithIsin: Asset = { ...mockAsset, isin: "US0378331005" };
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: assetWithIsin, onClose }));
    expect(result.current.formData.isin).toBe("US0378331005");
  });

  // AST-023 — asset with no ISIN initialises form field to empty string
  it("initialises isin to empty string when asset.isin is null", () => {
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: mockAsset, onClose }));
    expect(result.current.formData.isin).toBe("");
  });

  // AST-023 — entering an ISIN trims whitespace and forwards the normalized value
  it("trims and forwards a non-empty isin to the gateway on submit", async () => {
    mockUpdateAsset.mockResolvedValue({ data: mockAsset, error: null });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: mockAsset, onClose }));

    act(() => {
      result.current.handleChange({
        target: { name: "isin", value: "  US0378331005  " },
      } as React.ChangeEvent<HTMLInputElement>);
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpdateAsset).toHaveBeenCalledWith(expect.objectContaining({ isin: "US0378331005" }));
  });

  // AST-023 — clearing the ISIN field submits isin: null
  it("submits isin: null when the ISIN field is cleared", async () => {
    const assetWithIsin: Asset = { ...mockAsset, isin: "US0378331005" };
    mockUpdateAsset.mockResolvedValue({ data: assetWithIsin, error: null });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: assetWithIsin, onClose }));

    act(() => {
      result.current.handleChange({
        target: { name: "isin", value: "" },
      } as React.ChangeEvent<HTMLInputElement>);
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpdateAsset).toHaveBeenCalledWith(expect.objectContaining({ isin: null }));
  });

  // AST-024 — editing an asset pre-fills the interest_bearing checkbox from the asset
  it("pre-fills interest_bearing from the asset", () => {
    const interestBearingAsset: Asset = { ...mockAsset, interest_bearing: true };
    const onClose = vi.fn();
    const { result } = renderHook(() =>
      useEditAssetModal({ asset: interestBearingAsset, onClose }),
    );
    expect(result.current.formData.interest_bearing).toBe(true);
  });

  // AST-024 — toggling the checkbox submits the new interest_bearing value
  it("submits the toggled interest_bearing value on submit", async () => {
    mockUpdateAsset.mockResolvedValue({ data: mockAsset, error: null });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: mockAsset, onClose }));

    expect(result.current.formData.interest_bearing).toBe(false);

    act(() => {
      result.current.handleChange({
        target: { name: "interest_bearing", value: "on", type: "checkbox", checked: true },
      } as unknown as React.ChangeEvent<HTMLInputElement>);
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpdateAsset).toHaveBeenCalledWith(
      expect.objectContaining({ interest_bearing: true }),
    );
  });

  // AST-022 — changing the picker submits the new exchange
  it("submits the new exchange when picker value changes (AST-022 — change)", async () => {
    const oldExchange: Exchange = { code: "XPAR", label: "Euronext Paris" };
    const newExchange: Exchange = { code: "XNAS", label: "NASDAQ" };
    const assetWithExchange: Asset = { ...mockAsset, exchange: oldExchange };
    mockUpdateAsset.mockResolvedValue({ data: assetWithExchange, error: null });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAssetModal({ asset: assetWithExchange, onClose }));

    act(() => {
      result.current.handleExchangeChange(newExchange);
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpdateAsset).toHaveBeenCalledWith(
      expect.objectContaining({ exchange: newExchange }),
    );
  });
});
