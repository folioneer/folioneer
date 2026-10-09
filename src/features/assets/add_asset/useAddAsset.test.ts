import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Asset, AssetKind, AssetLookupResult, Exchange } from "@/bindings";
import { DEFAULT_CATEGORY_ID, defaultRiskOf, kindFormOf } from "../shared/creationDefaults";
import { useAddAsset } from "./useAddAsset";

const mockAddAsset = vi.fn();
const mockProposeAssetReference = vi.fn();

vi.mock("../useAssets", () => ({
  useAssets: () => ({
    addAsset: mockAddAsset,
    assets: [] as Asset[],
    activeCount: 0,
    loading: false,
    fetchError: null,
    fetchAssets: vi.fn(),
    updateAsset: vi.fn(),
    archiveAsset: vi.fn(),
    unarchiveAsset: vi.fn(),
    deleteAsset: vi.fn(),
  }),
}));

vi.mock("../gateway", () => ({
  assetGateway: {
    proposeAssetReference: (name: string) => mockProposeAssetReference(name),
  },
}));

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;
const xnas: Exchange = { code: "XNAS", label: "Nasdaq" };

const lookupResult: AssetLookupResult = {
  name: "Apple Inc.",
  reference: "AAPL",
  isin: "US0378331005",
  currency: "USD",
  asset_class: "Stocks",
  exchange: xnas,
};

const type = (result: { current: ReturnType<typeof useAddAsset> }, name: string, value: string) => {
  act(() => {
    result.current.handleChange({
      target: { name, value },
    } as React.ChangeEvent<HTMLInputElement>);
  });
};

describe("useAddAsset", () => {
  beforeEach(() => {
    mockAddAsset.mockReset();
    mockProposeAssetReference.mockReset();
    mockProposeAssetReference.mockImplementation(async (name: string) =>
      name.toUpperCase().replace(/ /g, "-"),
    );
  });

  // R2, AST-037 — a new asset starts in the default category and in the class the core
  // preselects for its kind, with that class's risk level.
  it("starts from what the core preselects for the kind", () => {
    for (const kind of ["Listed", "Crypto", "Custom"] as AssetKind[]) {
      const { result } = renderHook(() => useAddAsset({ kind }));
      const form = kindFormOf(kind);
      expect(result.current.formData.kind).toBe(kind);
      expect(result.current.formData.category_id).toBe(DEFAULT_CATEGORY_ID);
      expect(result.current.formData.class).toBe(form?.class);
      expect(result.current.formData.risk_level).toBe(defaultRiskOf(form?.class ?? "Stocks"));
      expect(result.current.formData.exchange).toBeNull();
      expect(result.current.formData.interest_bearing).toBe(false);
    }
  });

  // R10 — risk_level auto-filled when class changes
  it("auto-fills risk_level when class changes", () => {
    const { result } = renderHook(() => useAddAsset({ kind: "Listed" }));

    act(() => {
      result.current.handleClassChange("Bonds");
    });

    expect(result.current.formData.class).toBe("Bonds");
    expect(result.current.formData.risk_level).toBe(defaultRiskOf("Bonds"));
  });

  // AST-037 — changing the kind keeps what was typed and swaps a class the new kind does
  // not offer for the kind's own, with its risk level.
  it("keeps what was typed and swaps the class when the kind changes", () => {
    const { result, rerender } = renderHook(({ kind }) => useAddAsset({ kind }), {
      initialProps: { kind: "Listed" as AssetKind },
    });
    type(result, "name", "Bitcoin");
    act(() => {
      result.current.handleClassChange("Bonds");
    });

    rerender({ kind: "Custom" });
    expect(result.current.formData.class).toBe("Bonds");
    expect(result.current.formData.name).toBe("Bitcoin");

    rerender({ kind: "Crypto" });
    expect(result.current.formData.class).toBe("DigitalAsset");
    expect(result.current.formData.risk_level).toBe(defaultRiskOf("DigitalAsset"));
    expect(result.current.formData.name).toBe("Bitcoin");
  });

  // AST-038 — a custom asset's reference is proposed from its name by the core until the
  // user types one; no other kind proposes one.
  it("proposes a custom asset's reference from its name until one is typed", async () => {
    const { result } = renderHook(() => useAddAsset({ kind: "Custom" }));
    type(result, "name", "Flat Paris");
    await waitFor(() => expect(result.current.formData.reference).toBe("FLAT-PARIS"));
    expect(result.current.referenceProposed).toBe(true);

    type(result, "reference", "MINE");
    type(result, "name", "Flat Paris 15");
    expect(result.current.referenceProposed).toBe(false);
    await waitFor(() => expect(mockProposeAssetReference).toHaveBeenCalledTimes(2));
    expect(result.current.formData.reference).toBe("MINE");
  });

  it("drops a proposed reference when the kind stops proposing one, never a typed one", async () => {
    const { result, rerender } = renderHook(
      ({ kind }: { kind: AssetKind }) => useAddAsset({ kind }),
      {
        initialProps: { kind: "Custom" as AssetKind },
      },
    );
    type(result, "name", "Flat Paris");
    await waitFor(() => expect(result.current.formData.reference).toBe("FLAT-PARIS"));

    rerender({ kind: "Crypto" });
    await waitFor(() => expect(result.current.formData.reference).toBe(""));
    expect(result.current.formData.name).toBe("Flat Paris");

    type(result, "reference", "MINE");
    rerender({ kind: "Listed" });
    expect(result.current.formData.reference).toBe("MINE");
  });

  it("proposes no reference for a listed or a crypto asset", async () => {
    for (const kind of ["Listed", "Crypto"] as AssetKind[]) {
      const { result } = renderHook(() => useAddAsset({ kind }));
      type(result, "name", "Something");
      expect(result.current.referenceProposed).toBe(false);
      expect(result.current.formData.reference).toBe("");
    }
    expect(mockProposeAssetReference).not.toHaveBeenCalled();
  });

  // AST-031 — the kind is sent, with only the fields that kind has.
  it("sends a listed asset with its ISIN trimmed and its exchange", async () => {
    mockAddAsset.mockResolvedValue({ data: { id: "new" }, error: null });
    const { result } = renderHook(() => useAddAsset({ kind: "Listed", prefill: lookupResult }));
    type(result, "isin", "  US0378331005  ");

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockAddAsset).toHaveBeenCalledWith({
      kind: "Listed",
      name: "Apple Inc.",
      reference: "AAPL",
      isin: "US0378331005",
      class: "Stocks",
      currency: "USD",
      risk_level: defaultRiskOf("Stocks"),
      category_id: DEFAULT_CATEGORY_ID,
      exchange: xnas,
      interest_bearing: false,
    });
  });

  it("sends an empty ISIN as none", async () => {
    mockAddAsset.mockResolvedValue({ data: { id: "new" }, error: null });
    const { result } = renderHook(() => useAddAsset({ kind: "Listed" }));
    type(result, "isin", "   ");

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockAddAsset).toHaveBeenCalledWith(expect.objectContaining({ isin: null }));
  });

  it("sends neither ISIN nor exchange for a kind that has none", async () => {
    mockAddAsset.mockResolvedValue({ data: { id: "new" }, error: null });
    const { result, rerender } = renderHook(
      ({ kind }) => useAddAsset({ kind, prefill: lookupResult }),
      { initialProps: { kind: "Listed" as AssetKind } },
    );
    act(() => {
      result.current.handleChange({
        target: { name: "interest_bearing", type: "checkbox", checked: true },
      } as unknown as React.ChangeEvent<HTMLInputElement>);
    });

    rerender({ kind: "Custom" });
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(mockAddAsset).toHaveBeenLastCalledWith(
      expect.objectContaining({
        kind: "Custom",
        isin: null,
        exchange: null,
        interest_bearing: true,
      }),
    );
  });

  it("never marks a crypto asset as bearing interest", async () => {
    mockAddAsset.mockResolvedValue({ data: { id: "new" }, error: null });
    const { result, rerender } = renderHook(({ kind }) => useAddAsset({ kind }), {
      initialProps: { kind: "Custom" as AssetKind },
    });
    act(() => {
      result.current.handleChange({
        target: { name: "interest_bearing", type: "checkbox", checked: true },
      } as unknown as React.ChangeEvent<HTMLInputElement>);
    });
    rerender({ kind: "Crypto" });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockAddAsset).toHaveBeenLastCalledWith(
      expect.objectContaining({ kind: "Crypto", class: "DigitalAsset", interest_bearing: false }),
    );
  });

  // WEB-041 — a lookup result fills the form; a class it does not carry is the kind's.
  it("fills the form from a lookup result", () => {
    const { result } = renderHook(() =>
      useAddAsset({ kind: "Listed", prefill: { ...lookupResult, asset_class: null } }),
    );
    expect(result.current.formData.name).toBe("Apple Inc.");
    expect(result.current.formData.reference).toBe("AAPL");
    expect(result.current.formData.isin).toBe("US0378331005");
    expect(result.current.formData.currency).toBe("USD");
    expect(result.current.formData.exchange).toEqual(xnas);
    expect(result.current.formData.class).toBe(kindFormOf("Listed")?.class);
  });

  // R14 — the dialog stays open on a refusal and shows it.
  it("does not call onSubmitSuccess and exposes the refusal", async () => {
    const refusal = { key: "error.AssetAlreadyExists", vars: { existing_name: "ASML" } };
    mockAddAsset.mockResolvedValue({ data: null, error: refusal });
    const onSubmitSuccess = vi.fn();
    const { result, rerender } = renderHook(({ kind }) => useAddAsset({ kind, onSubmitSuccess }), {
      initialProps: { kind: "Listed" as AssetKind },
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(result.current.error).toEqual(refusal);
    expect(onSubmitSuccess).not.toHaveBeenCalled();

    rerender({ kind: "Custom" });
    expect(result.current.error).toBeNull();
  });

  // R14 — success hands the new asset over and empties the form.
  it("calls onSubmitSuccess and empties the form on success", async () => {
    mockAddAsset.mockResolvedValue({ data: { id: "new" }, error: null });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() =>
      useAddAsset({ kind: "Listed", prefill: lookupResult, onSubmitSuccess }),
    );

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(onSubmitSuccess).toHaveBeenCalledWith("new");
    expect(result.current.error).toBeNull();
    expect(result.current.formData.name).toBe("");
    expect(result.current.formData.exchange).toBeNull();
    expect(result.current.formData.interest_bearing).toBe(false);
  });
});
