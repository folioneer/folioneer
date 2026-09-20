import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Asset } from "@/bindings";
import { useAppStore } from "@/lib/store";
import { AssetEditModalMount } from "./AssetEditModalMount";

const navigateMock = vi.fn();
let searchValue: Record<string, unknown> = {};

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => navigateMock,
  useSearch: () => searchValue,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en-US" },
  }),
}));

vi.mock("@/features/assets/edit_asset_modal/EditAssetModal", () => ({
  EditAssetModal: ({
    isOpen,
    onClose,
    asset,
    focusField,
  }: {
    isOpen: boolean;
    onClose: () => void;
    asset: Asset | null;
    focusField?: string;
  }) =>
    isOpen ? (
      <div
        data-testid="edit-asset-modal-stub"
        data-asset-id={asset?.id ?? "none"}
        data-focus={focusField ?? "none"}
      >
        <button type="button" data-testid="stub-close-button" onClick={onClose}>
          close
        </button>
      </div>
    ) : null,
}));

const seedAsset: Asset = {
  id: "asset-42",
  name: "Acme",
  reference: "",
  category: { id: "cat-1", name: "Default" },
  currency: "USD",
  risk_level: 3,
  class: "Stocks",
  isin: null,
  is_archived: false,
  price_refresh_blocked: false,
  interest_bearing: false,
  exchange: null,
};

describe("AssetEditModalMount", () => {
  beforeEach(() => {
    navigateMock.mockClear();
    searchValue = {};
    useAppStore.setState({ assets: [seedAsset] });
  });

  it("renders nothing when modal param is absent", () => {
    searchValue = {};
    render(<AssetEditModalMount />);
    expect(screen.queryByTestId("edit-asset-modal-stub")).toBeNull();
  });

  it("renders nothing when modal param is not 'edit-asset'", () => {
    searchValue = { modal: "some-other", editAssetId: "asset-42" };
    render(<AssetEditModalMount />);
    expect(screen.queryByTestId("edit-asset-modal-stub")).toBeNull();
  });

  it("renders nothing when editAssetId resolves to no asset", () => {
    searchValue = { modal: "edit-asset", editAssetId: "missing-id" };
    render(<AssetEditModalMount />);
    expect(screen.queryByTestId("edit-asset-modal-stub")).toBeNull();
  });

  it("mounts EditAssetModal with asset + focusField when params present", () => {
    searchValue = {
      modal: "edit-asset",
      editAssetId: "asset-42",
      focusField: "reference",
    };
    render(<AssetEditModalMount />);
    const stub = screen.getByTestId("edit-asset-modal-stub");
    expect(stub.dataset.assetId).toBe("asset-42");
    expect(stub.dataset.focus).toBe("reference");
  });

  it("ignores invalid focusField values", () => {
    searchValue = {
      modal: "edit-asset",
      editAssetId: "asset-42",
      focusField: "not-a-field",
    };
    render(<AssetEditModalMount />);
    expect(screen.getByTestId("edit-asset-modal-stub").dataset.focus).toBe("none");
  });

  it("close clears modal/editAssetId/focusField via navigate-replace", () => {
    searchValue = {
      modal: "edit-asset",
      editAssetId: "asset-42",
      focusField: "reference",
    };
    render(<AssetEditModalMount />);
    fireEvent.click(screen.getByTestId("stub-close-button"));
    expect(navigateMock).toHaveBeenCalledTimes(1);
    const call = navigateMock.mock.calls[0];
    if (!call) throw new Error("expected navigate to be called");
    const arg = call[0] as { search: (prev: object) => object; replace: boolean };
    expect(arg.replace).toBe(true);
    expect(arg.search({ existing: "kept" })).toEqual({
      existing: "kept",
      modal: undefined,
      editAssetId: undefined,
      focusField: undefined,
    });
  });
});
