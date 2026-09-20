import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AssetLookupResult, LookupMode } from "@/bindings";
import { SearchPanel } from "./SearchPanel";
import type { WebLookupSearchState } from "./useWebLookupSearch";

// Translate asset.class.* keys to English labels so result-row assertions read
// naturally. opts.name covers the select_result interpolation. Everything else
// passes through as the key (sufficient for all other assertions in this file).
const CLASS_LABELS: Record<string, string> = {
  "asset.class.Cash": "Cash",
  "asset.class.Bonds": "Bonds",
  "asset.class.RealEstate": "Real Estate",
  "asset.class.MutualFunds": "Mutual Funds",
  "asset.class.ETF": "ETF",
  "asset.class.Stocks": "Stocks",
  "asset.class.DigitalAsset": "Digital Asset",
  "asset.class.Derivatives": "Derivatives",
};

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, string>) => opts?.name ?? CLASS_LABELS[key] ?? key,
  }),
}));

const noop = () => {};
const noopSubmit = (_mode: LookupMode) => {};

interface PanelProps {
  isinQuery?: string;
  keywordQuery?: string;
  state?: WebLookupSearchState;
  lastMode?: LookupMode | null;
  onSelect?: (result: AssetLookupResult) => void;
  onFillManually?: () => void;
  submit?: (mode: LookupMode) => void;
  retry?: () => void;
  setIsinQuery?: (q: string) => void;
  setKeywordQuery?: (q: string) => void;
}

function renderPanel({
  isinQuery = "",
  keywordQuery = "",
  state = { status: "idle" },
  lastMode = null,
  onSelect = noop,
  onFillManually = noop,
  submit = noopSubmit,
  retry = noop,
  setIsinQuery = noop,
  setKeywordQuery = noop,
}: PanelProps = {}) {
  return render(
    <SearchPanel
      isinQuery={isinQuery}
      keywordQuery={keywordQuery}
      state={state}
      lastMode={lastMode}
      submit={submit}
      retry={retry}
      setIsinQuery={setIsinQuery}
      setKeywordQuery={setKeywordQuery}
      onSelect={onSelect}
      onFillManually={onFillManually}
    />,
  );
}

// ---------------------------------------------------------------------------
// Two-field layout (WEB-012)
// ---------------------------------------------------------------------------

describe("SearchPanel — two-field layout (WEB-012)", () => {
  // WEB-012 — both input fields are rendered
  it("renders the ISIN input field", () => {
    renderPanel();
    expect(screen.getByTestId("web-lookup-isin-input")).toBeInTheDocument();
  });

  it("renders the keyword input field", () => {
    renderPanel();
    expect(screen.getByTestId("web-lookup-keyword-input")).toBeInTheDocument();
  });

  // WEB-012 — both submit buttons are rendered
  it("renders the ISIN submit button", () => {
    renderPanel();
    expect(screen.getByTestId("web-lookup-isin-submit")).toBeInTheDocument();
  });

  it("renders the keyword submit button", () => {
    renderPanel();
    expect(screen.getByTestId("web-lookup-keyword-submit")).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Per-field enable rule (WEB-011)
// ---------------------------------------------------------------------------

describe("SearchPanel — per-field submit enable (WEB-011)", () => {
  // WEB-011 — ISIN button disabled when ISIN field is empty
  it("ISIN submit button is disabled when isinQuery is empty", () => {
    renderPanel({ isinQuery: "" });
    expect(screen.getByTestId("web-lookup-isin-submit")).toBeDisabled();
  });

  // WEB-011 — ISIN button enabled when ISIN field has content
  it("ISIN submit button is enabled when isinQuery is non-empty", () => {
    renderPanel({ isinQuery: "IE00B53L3W79" });
    expect(screen.getByTestId("web-lookup-isin-submit")).not.toBeDisabled();
  });

  // WEB-011 — Keyword button disabled when keyword field is empty
  it("keyword submit button is disabled when keywordQuery is empty", () => {
    renderPanel({ keywordQuery: "" });
    expect(screen.getByTestId("web-lookup-keyword-submit")).toBeDisabled();
  });

  // WEB-011 — Keyword button enabled when keyword field has content
  it("keyword submit button is enabled when keywordQuery is non-empty", () => {
    renderPanel({ keywordQuery: "Apple" });
    expect(screen.getByTestId("web-lookup-keyword-submit")).not.toBeDisabled();
  });
});

// ---------------------------------------------------------------------------
// Submit dispatching (WEB-014)
// ---------------------------------------------------------------------------

describe("SearchPanel — submit dispatching (WEB-014)", () => {
  // Clicking ISIN submit calls submit("Isin")
  it("clicking ISIN submit calls submit with Isin mode", () => {
    const submit = vi.fn();
    renderPanel({ isinQuery: "IE00B53L3W79", submit });
    fireEvent.click(screen.getByTestId("web-lookup-isin-submit"));
    expect(submit).toHaveBeenCalledWith("Isin");
  });

  // Clicking Keyword submit calls submit("Keyword")
  it("clicking keyword submit calls submit with Keyword mode", () => {
    const submit = vi.fn();
    renderPanel({ keywordQuery: "Apple", submit });
    fireEvent.click(screen.getByTestId("web-lookup-keyword-submit"));
    expect(submit).toHaveBeenCalledWith("Keyword");
  });

  // Typing in the ISIN field forwards each keystroke to setIsinQuery
  it("typing in the ISIN field calls setIsinQuery with the new value", () => {
    const setIsinQuery = vi.fn();
    renderPanel({ setIsinQuery });
    fireEvent.change(screen.getByTestId("web-lookup-isin-input"), {
      target: { value: "IE00B53L3W79" },
    });
    expect(setIsinQuery).toHaveBeenCalledWith("IE00B53L3W79");
  });

  // Typing in the keyword field forwards each keystroke to setKeywordQuery
  it("typing in the keyword field calls setKeywordQuery with the new value", () => {
    const setKeywordQuery = vi.fn();
    renderPanel({ setKeywordQuery });
    fireEvent.change(screen.getByTestId("web-lookup-keyword-input"), {
      target: { value: "Apple" },
    });
    expect(setKeywordQuery).toHaveBeenCalledWith("Apple");
  });
});

// ---------------------------------------------------------------------------
// Per-field loading state (WEB-030)
// ---------------------------------------------------------------------------

describe("SearchPanel — per-field loading (WEB-030)", () => {
  // WEB-030 — when loading with lastMode=Isin, loading indicator is near the ISIN field
  it("shows ISIN loading indicator when lastMode is Isin and state is loading", () => {
    renderPanel({ state: { status: "loading" }, lastMode: "Isin" });
    expect(screen.getByTestId("web-lookup-isin-loading")).toBeInTheDocument();
  });

  // WEB-030 — when loading with lastMode=Keyword, loading indicator is near the Keyword field
  it("shows keyword loading indicator when lastMode is Keyword and state is loading", () => {
    renderPanel({ state: { status: "loading" }, lastMode: "Keyword" });
    expect(screen.getByTestId("web-lookup-keyword-loading")).toBeInTheDocument();
  });

  // WEB-030 — while ISIN is loading, the Keyword submit button stays enabled if keywordQuery non-empty
  it("keyword submit stays enabled while ISIN search is loading", () => {
    renderPanel({
      state: { status: "loading" },
      lastMode: "Isin",
      isinQuery: "IE00B53L3W79",
      keywordQuery: "Apple",
    });
    expect(screen.getByTestId("web-lookup-keyword-submit")).not.toBeDisabled();
  });

  // WEB-030 — while Keyword is loading, the ISIN submit button stays enabled if isinQuery non-empty
  it("ISIN submit stays enabled while keyword search is loading", () => {
    renderPanel({
      state: { status: "loading" },
      lastMode: "Keyword",
      isinQuery: "IE00B53L3W79",
      keywordQuery: "Apple",
    });
    expect(screen.getByTestId("web-lookup-isin-submit")).not.toBeDisabled();
  });
});

// ---------------------------------------------------------------------------
// Per-field inline error (WEB-033)
// ---------------------------------------------------------------------------

describe("SearchPanel — per-field inline error (WEB-033)", () => {
  // WEB-033 — InvalidIsinFormat renders beside the ISIN field when lastMode is Isin
  it("shows InvalidIsinFormat error beside the ISIN field when lastMode is Isin", () => {
    renderPanel({
      state: { status: "error", code: "InvalidIsinFormat" },
      lastMode: "Isin",
    });
    expect(screen.getByTestId("web-lookup-isin-error")).toBeInTheDocument();
    expect(screen.getByText("asset.web_lookup.error_invalid_isin")).toBeInTheDocument();
  });

  // WEB-033 — InvalidIsinFormat error does NOT appear beside the Keyword field
  it("does not render ISIN error beside the keyword field", () => {
    renderPanel({
      state: { status: "error", code: "InvalidIsinFormat" },
      lastMode: "Isin",
    });
    expect(screen.queryByTestId("web-lookup-keyword-error")).not.toBeInTheDocument();
  });

  // WEB-033 — Keyword submit button stays enabled when ISIN error is displayed
  it("keyword submit stays enabled when an ISIN error is shown", () => {
    renderPanel({
      state: { status: "error", code: "InvalidIsinFormat" },
      lastMode: "Isin",
      keywordQuery: "Apple",
    });
    expect(screen.getByTestId("web-lookup-keyword-submit")).not.toBeDisabled();
  });

  // WEB-033 — NetworkError renders beside the field that triggered it (Keyword path)
  it("shows NetworkError beside the keyword field when lastMode is Keyword", () => {
    const retry = vi.fn();
    renderPanel({
      state: { status: "error", code: "NetworkError" },
      lastMode: "Keyword",
      retry,
    });
    expect(screen.getByTestId("web-lookup-keyword-error")).toBeInTheDocument();
    expect(screen.getByText("asset.web_lookup.error_network")).toBeInTheDocument();
  });

  // WEB-033 — RateLimited renders beside the field that triggered it (Keyword path)
  it("shows RateLimited error beside the keyword field when lastMode is Keyword", () => {
    renderPanel({
      state: { status: "error", code: "RateLimited" },
      lastMode: "Keyword",
    });
    expect(screen.getByTestId("web-lookup-keyword-error")).toBeInTheDocument();
    expect(screen.getByText("asset.web_lookup.error_rate_limit")).toBeInTheDocument();
  });

  // WEB-033 — NetworkError on ISIN path renders beside ISIN field
  it("shows NetworkError beside the ISIN field when lastMode is Isin", () => {
    renderPanel({
      state: { status: "error", code: "NetworkError" },
      lastMode: "Isin",
    });
    expect(screen.getByTestId("web-lookup-isin-error")).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Status states (existing behaviors preserved)
// ---------------------------------------------------------------------------

describe("SearchPanel — status states", () => {
  // WEB-011 — idle hint shown when no search has been made
  it("shows idle hint in idle state", () => {
    renderPanel({ state: { status: "idle" } });
    expect(screen.getByText("asset.web_lookup.idle_hint")).toBeInTheDocument();
  });

  // WEB-032 — empty state shown when no results returned
  it("shows no-results message in empty state", () => {
    renderPanel({ state: { status: "empty" } });
    expect(screen.getByText("asset.web_lookup.no_results")).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Results row layout (WEB-031)
// ---------------------------------------------------------------------------

describe("SearchPanel — result row layout (WEB-031)", () => {
  const stockResult: AssetLookupResult = {
    name: "Apple Inc.",
    reference: "AAPL",
    isin: null,
    currency: "USD",
    asset_class: "Stocks",
    exchange: { code: "XNYS", label: "New York Stock Exchange" },
  };

  // WEB-031 — first line shows reference code and instrument name
  it("shows reference code and name on the first line", () => {
    renderPanel({ state: { status: "results", results: [stockResult] } });
    expect(screen.getByText("AAPL")).toBeInTheDocument();
    expect(screen.getByText("Apple Inc.")).toBeInTheDocument();
  });

  // WEB-031 — second line shows formatted class label and exchange separated by ·
  it("shows class label · exchange on the second line when both present", () => {
    renderPanel({ state: { status: "results", results: [stockResult] } });
    expect(screen.getByText("Stocks · New York Stock Exchange")).toBeInTheDocument();
  });

  // WEB-046 — reference prefix omitted when reference is absent
  it("omits the reference prefix when reference is null", () => {
    const noRef: AssetLookupResult = {
      name: "No Reference Fund",
      reference: null,
      isin: null,
      currency: "EUR",
      asset_class: "MutualFunds",
      exchange: null,
    };
    renderPanel({ state: { status: "results", results: [noRef] } });
    expect(screen.getByText("No Reference Fund")).toBeInTheDocument();
    expect(screen.queryByText("null")).toBeNull();
  });

  // WEB-040 — clicking a result calls onSelect with the result
  it("calls onSelect with the result when a row is clicked", () => {
    const onSelect = vi.fn();
    renderPanel({ state: { status: "results", results: [stockResult] }, onSelect });
    fireEvent.click(screen.getByRole("button", { name: "Apple Inc." }));
    expect(onSelect).toHaveBeenCalledWith(stockResult);
  });

  // WEB-031 — falls back to "Unknown type" label when asset_class is absent
  it("uses the type_unknown fallback when asset_class is null", () => {
    const unclassified: AssetLookupResult = {
      name: "Mystery Instrument",
      reference: "MYST",
      isin: null,
      currency: "USD",
      asset_class: null,
      exchange: null,
    };
    renderPanel({ state: { status: "results", results: [unclassified] } });
    expect(screen.getByText("asset.web_lookup.type_unknown")).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// WEB-013 — Fill manually bypass
// ---------------------------------------------------------------------------

describe("SearchPanel — fill manually bypass (WEB-013)", () => {
  it("fill manually button is always visible in idle state", () => {
    renderPanel({ state: { status: "idle" } });
    expect(
      screen.getByRole("button", {
        name: "asset.web_lookup.action_fill_manually",
      }),
    ).toBeInTheDocument();
  });

  it("fill manually button is visible even in error state", () => {
    renderPanel({ state: { status: "error", code: "NetworkError" }, lastMode: "Keyword" });
    expect(
      screen.getByRole("button", {
        name: "asset.web_lookup.action_fill_manually",
      }),
    ).toBeInTheDocument();
  });
});
