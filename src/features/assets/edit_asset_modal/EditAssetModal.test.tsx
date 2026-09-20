import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Asset } from "@/bindings";
import { EditAssetModal } from "./EditAssetModal";

vi.mock("./useEditAssetModal", () => ({
  useEditAssetModal: () => ({
    formData: {
      name: "",
      reference: "",
      isin: "",
      class: "Stocks",
      currency: "USD",
      risk_level: 3,
      category_id: "cat-1",
      exchange: null,
    },
    error: null,
    isSubmitting: false,
    duplicateWarning: false,
    handleChange: vi.fn(),
    handleClassChange: vi.fn(),
    handleExchangeChange: vi.fn(),
    handleSubmit: vi.fn(),
    categories: [],
  }),
}));

vi.mock("../shared/AssetForm", () => ({
  AssetForm: ({ idPrefix = "asset" }: { idPrefix?: string }) => (
    <>
      <input id={`${idPrefix}-reference`} data-testid="reference-input" />
      <input id={`${idPrefix}-isin`} data-testid="isin-input" />
    </>
  ),
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn() },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en-US" },
  }),
}));

const baseAsset: Asset = {
  id: "asset-1",
  name: "Apple Inc.",
  reference: "AAPL",
  isin: null,
  class: "Stocks",
  currency: "USD",
  risk_level: 4,
  category: { id: "cat-1", name: "Stocks" },
  is_archived: false,
  price_refresh_blocked: false,
  interest_bearing: false,
  exchange: null,
};

describe("EditAssetModal — focusField effect (MKT-032)", () => {
  it("focuses the reference input when focusField='reference'", () => {
    vi.useFakeTimers();
    render(
      <EditAssetModal isOpen={true} onClose={vi.fn()} asset={baseAsset} focusField="reference" />,
    );
    vi.runAllTimers();
    const ref = document.getElementById("edit-asset-reference");
    expect(document.activeElement).toBe(ref);
    vi.useRealTimers();
  });

  it("focuses the isin input when focusField='isin'", () => {
    vi.useFakeTimers();
    render(<EditAssetModal isOpen={true} onClose={vi.fn()} asset={baseAsset} focusField="isin" />);
    vi.runAllTimers();
    const isin = document.getElementById("edit-asset-isin");
    expect(document.activeElement).toBe(isin);
    vi.useRealTimers();
  });

  it("does not move focus when focusField is undefined", () => {
    vi.useFakeTimers();
    render(<EditAssetModal isOpen={true} onClose={vi.fn()} asset={baseAsset} />);
    vi.runAllTimers();
    const ref = document.getElementById("edit-asset-reference");
    const isin = document.getElementById("edit-asset-isin");
    expect(document.activeElement).not.toBe(ref);
    expect(document.activeElement).not.toBe(isin);
    vi.useRealTimers();
  });

  it("does not move focus when isOpen is false", () => {
    vi.useFakeTimers();
    const { container } = render(
      <EditAssetModal isOpen={false} onClose={vi.fn()} asset={baseAsset} focusField="reference" />,
    );
    vi.runAllTimers();
    expect(container.querySelector("#edit-asset-reference")).toBeNull();
    vi.useRealTimers();
  });
});
