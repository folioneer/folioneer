import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/lib/store";
import { PriceFreshnessIndicator } from "./PriceFreshnessIndicator";

const { mockUsePriceFreshness } = vi.hoisted(() => ({ mockUsePriceFreshness: vi.fn() }));

vi.mock("./usePriceFreshness", () => ({ usePriceFreshness: () => mockUsePriceFreshness() }));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
    i18n: { language: "en" },
  }),
}));

describe("PriceFreshnessIndicator (MKT-202)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("names its subject and shows the newest price date", () => {
    mockUsePriceFreshness.mockReturnValue({
      visible: true,
      newestPriceDate: "2026-09-15",
      lastFetchAt: "2026-08-28T09:30:28",
    });
    render(<PriceFreshnessIndicator />);

    expect(screen.getByTestId("price-freshness")).toHaveTextContent(
      'price_freshness.as_of:{"date":"9/15/2026"}',
    );
  });

  it("tells on hover when this computer last fetched prices", () => {
    mockUsePriceFreshness.mockReturnValue({
      visible: true,
      newestPriceDate: "2026-09-15",
      lastFetchAt: "2026-08-28T09:30:28",
    });
    render(<PriceFreshnessIndicator />);

    expect(screen.getByTestId("price-freshness")).toHaveAttribute(
      "title",
      'price_freshness.last_fetch:{"when":"8/28/2026 9:30 AM"}',
    );
  });

  it("tells on hover that this computer never fetched prices", () => {
    mockUsePriceFreshness.mockReturnValue({
      visible: true,
      newestPriceDate: "2026-09-15",
      lastFetchAt: null,
    });
    render(<PriceFreshnessIndicator />);

    expect(screen.getByTestId("price-freshness")).toHaveAttribute(
      "title",
      "price_freshness.never_fetched",
    );
  });

  it("is absent until its figures have been read", () => {
    mockUsePriceFreshness.mockReturnValue({
      visible: false,
      newestPriceDate: null,
      lastFetchAt: null,
    });
    render(<PriceFreshnessIndicator />);

    expect(screen.queryByTestId("price-freshness")).toBeNull();
  });

  it("says that no price is recorded yet when there is no date", () => {
    mockUsePriceFreshness.mockReturnValue({
      visible: true,
      newestPriceDate: null,
      lastFetchAt: null,
    });
    render(<PriceFreshnessIndicator />);

    expect(screen.getByTestId("price-freshness")).toHaveTextContent("price_freshness.none");
  });
});

describe("PriceFreshnessIndicator — without an External provider (MKT-212)", () => {
  afterEach(() => {
    useAppStore.setState({ hasExternalProvider: true });
  });

  // Prices typed by hand are recorded prices: the date stays, the fetch line goes.
  it("keeps the newest price date and says nothing about fetching", () => {
    useAppStore.setState({ hasExternalProvider: false });
    mockUsePriceFreshness.mockReturnValue({
      visible: true,
      newestPriceDate: "2026-09-15",
      lastFetchAt: null,
    });
    render(<PriceFreshnessIndicator />);

    const item = screen.getByTestId("price-freshness");
    expect(item).toHaveTextContent('price_freshness.as_of:{"date":"9/15/2026"}');
    expect(item).not.toHaveAttribute("title");
  });
});
