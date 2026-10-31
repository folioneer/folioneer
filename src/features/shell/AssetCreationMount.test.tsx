import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAssetCreationRequest } from "@/lib/assetCreationRequest";
import { AssetCreationMount } from "./AssetCreationMount";

const onCreated = vi.fn();
vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn() } }));
vi.mock("@/features/assets/add_asset/AddAsset", () => ({
  AddAssetModal: ({
    prefill,
    kind,
    onKindChange,
    onSuccess,
    onClose,
  }: {
    prefill: { name: string };
    kind: string;
    onKindChange: (kind: string) => void;
    onSuccess: (id: string) => void;
    onClose: () => void;
  }) => (
    <>
      <button type="button" data-testid="kind" onClick={() => onKindChange("Crypto")}>
        {kind}
      </button>
      <button
        type="button"
        onClick={() => {
          onSuccess("created");
          onClose();
        }}
      >
        {prefill.name}
      </button>
    </>
  ),
}));

const prefill = {
  name: "Exemple",
  reference: null,
  isin: null,
  currency: "EUR",
  asset_class: null,
  exchange: null,
};

describe("AssetCreationMount", () => {
  beforeEach(() => {
    onCreated.mockReset();
    act(() => useAssetCreationRequest.getState().close());
  });

  it("shows nothing without a request", () => {
    const { container } = render(<AssetCreationMount />);
    expect(container).toBeEmptyDOMElement();
  });

  // IMP-052 — the usual form, prefilled; the id goes back to the one who asked.
  it("hosts the form for a request, gives the id back and closes", () => {
    render(<AssetCreationMount />);
    act(() => useAssetCreationRequest.getState().open({ prefill, onCreated }));
    act(() => screen.getByText("Exemple").click());
    expect(onCreated).toHaveBeenCalledWith("created");
    expect(useAssetCreationRequest.getState().request).toBeNull();
  });

  it("starts each request from the kind its prefill can fill", () => {
    render(<AssetCreationMount />);
    const open = (isin: string | null) =>
      act(() =>
        useAssetCreationRequest.getState().open({ prefill: { ...prefill, isin }, onCreated }),
      );
    open(null);
    expect(screen.getByTestId("kind").textContent).toBe("Custom");
    fireEvent.click(screen.getByTestId("kind"));
    expect(screen.getByTestId("kind").textContent).toBe("Crypto");
    act(() => useAssetCreationRequest.getState().close());
    open("FR0000000001");
    expect(screen.getByTestId("kind").textContent).toBe("Listed");
  });
});
