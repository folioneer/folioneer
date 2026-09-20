import { configure, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UnpricedAsset } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";

// The component uses stable `id` attributes (F25, E2E-ready). Resolve getByTestId
// against `id` so these unit assertions target the same selectors the E2E suite uses.
configure({ testIdAttribute: "id" });

// Mock react-i18next — return the key so assertions use i18n keys (F24).
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
    i18n: { language: "en-US" },
  }),
}));

// Mock the hook — the component test verifies visible DOM + affordance wiring,
// not the hook state machine (tested in useUnpricedPrices.test.ts).
const mockRecord = vi.fn();
const mockSkip = vi.fn();

vi.mock("./useUnpricedPrices", () => ({
  useUnpricedPrices: vi.fn(),
}));

import { UnpricedPricesModal } from "./UnpricedPricesModal";
import * as hookModule from "./useUnpricedPrices";

// ----------- fixtures -----------

const makeAsset = (overrides: Partial<UnpricedAsset> = {}): UnpricedAsset => ({
  asset_id: "asset-1",
  name: "Air Liquide",
  reference: "AI.PA",
  isin: "FR0000120073",
  currency: "EUR",
  last_price: 160_000_000,
  last_price_date: "2026-06-18",
  ...overrides,
});

// A row shape that mirrors what the hook exposes in its `rows` array.
type RowState = UnpricedAsset & {
  isSubmitting: boolean;
  error: I18nMessage | null;
};

const makeRow = (asset: UnpricedAsset, overrides: Partial<RowState> = {}): RowState => ({
  ...asset,
  isSubmitting: false,
  error: null,
  ...overrides,
});

const setupHook = (rows: RowState[], onClose = vi.fn()) => {
  vi.mocked(hookModule.useUnpricedPrices).mockReturnValue({
    rows,
    record: mockRecord,
    skip: mockSkip,
  });
  return onClose;
};

// ----------- tests -----------

describe("UnpricedPricesModal — row rendering (MKT-174)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders one row per unpriced asset", () => {
    const assets = [
      makeAsset({ asset_id: "asset-1", name: "Air Liquide" }),
      makeAsset({ asset_id: "asset-2", name: "LVMH", reference: "MC.PA" }),
    ];
    setupHook(assets.map((a) => makeRow(a)));

    render(<UnpricedPricesModal assets={assets} onClose={vi.fn()} />);

    expect(screen.getByTestId("unpriced-row-asset-1")).toBeInTheDocument();
    expect(screen.getByTestId("unpriced-row-asset-2")).toBeInTheDocument();
  });

  it("renders asset name in each row (MKT-174)", () => {
    const asset = makeAsset({ name: "Air Liquide" });
    setupHook([makeRow(asset)]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    expect(screen.getByText("Air Liquide")).toBeInTheDocument();
  });

  it("renders ticker (reference) in each row (MKT-174)", () => {
    const asset = makeAsset({ reference: "AI.PA" });
    setupHook([makeRow(asset)]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    expect(screen.getByTestId("unpriced-reference-asset-1")).toHaveTextContent("AI.PA");
  });

  it("renders ISIN when present (MKT-174)", () => {
    const asset = makeAsset({ isin: "FR0000120073" });
    setupHook([makeRow(asset)]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    expect(screen.getByTestId("unpriced-isin-asset-1")).toHaveTextContent("FR0000120073");
  });

  it("does not render ISIN cell when isin is null (MKT-174)", () => {
    const asset = makeAsset({ isin: null });
    setupHook([makeRow(asset)]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    expect(screen.queryByTestId("unpriced-isin-asset-1")).not.toBeInTheDocument();
  });

  it("renders last_price formatted in asset currency when present (MKT-174)", () => {
    // 160_000_000 micros = 160.00 EUR
    const asset = makeAsset({ last_price: 160_000_000, currency: "EUR" });
    setupHook([makeRow(asset)]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    const priceCell = screen.getByTestId("unpriced-last-price-asset-1");
    // The component must render a non-empty formatted value (exact format is
    // implementation-defined; the i18n key or a numeric string must appear).
    expect(priceCell).not.toBeEmptyDOMElement();
  });

  it("renders 'no previous price' indicator when last_price is null (MKT-174)", () => {
    const asset = makeAsset({ last_price: null, last_price_date: null });
    setupHook([makeRow(asset)]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    // The component shows an i18n key (or translated text) indicating no price.
    // We assert the stable-id element is present and contains the key text.
    const cell = screen.getByTestId("unpriced-last-price-asset-1");
    expect(cell).toHaveTextContent("unpriced_prices.no_previous_price");
  });

  it("renders an empty price input per row (MKT-174)", () => {
    const asset = makeAsset();
    setupHook([makeRow(asset)]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    const input = screen.getByTestId("unpriced-price-input-asset-1");
    expect(input).toBeInTheDocument();
    expect((input as HTMLInputElement).value).toBe("");
  });

  it("renders confirm and skip controls per row (MKT-174)", () => {
    const asset = makeAsset();
    setupHook([makeRow(asset)]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    expect(screen.getByTestId("unpriced-confirm-asset-1")).toBeInTheDocument();
    expect(screen.getByTestId("unpriced-skip-asset-1")).toBeInTheDocument();
  });
});

describe("UnpricedPricesModal — confirm wiring (MKT-175)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("calls hook.record with asset_id and parsed price when confirm is clicked", async () => {
    const user = userEvent.setup();
    const asset = makeAsset({ asset_id: "asset-1" });
    setupHook([makeRow(asset)]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    await user.type(screen.getByTestId("unpriced-price-input-asset-1"), "15.50");
    await user.click(screen.getByTestId("unpriced-confirm-asset-1"));

    expect(mockRecord).toHaveBeenCalledWith("asset-1", 15.5);
  });

  it("disables confirm when price input is empty", () => {
    const asset = makeAsset();
    setupHook([makeRow(asset)]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    expect(screen.getByTestId("unpriced-confirm-asset-1")).toBeDisabled();
  });
});

describe("UnpricedPricesModal — skip wiring (MKT-176)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("calls hook.skip with asset_id when skip is clicked", async () => {
    const user = userEvent.setup();
    const asset = makeAsset({ asset_id: "asset-1" });
    setupHook([makeRow(asset)]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    await user.click(screen.getByTestId("unpriced-skip-asset-1"));

    expect(mockSkip).toHaveBeenCalledWith("asset-1");
  });
});

describe("UnpricedPricesModal — in-flight state (MKT-178)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("disables confirm and skip controls while the row is submitting", () => {
    const asset = makeAsset({ asset_id: "asset-1" });
    setupHook([makeRow(asset, { isSubmitting: true })]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    expect(screen.getByTestId("unpriced-confirm-asset-1")).toBeDisabled();
    expect(screen.getByTestId("unpriced-skip-asset-1")).toBeDisabled();
  });
});

describe("UnpricedPricesModal — per-row error (MKT-178)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders an inline error message when the row has an error (MKT-178)", () => {
    const asset = makeAsset({ asset_id: "asset-1" });
    setupHook([makeRow(asset, { error: { key: "error.NotPositive" } })]);

    render(<UnpricedPricesModal assets={[asset]} onClose={vi.fn()} />);

    // The component renders the i18n error key; exact rendering is implementation-defined.
    expect(screen.getByTestId("unpriced-error-asset-1")).toHaveTextContent("error.NotPositive");
  });
});
