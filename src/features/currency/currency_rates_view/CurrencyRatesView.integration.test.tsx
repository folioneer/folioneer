import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as gateway from "../gateway";

vi.mock("../gateway");

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
    i18n: { language: "en" },
  }),
}));

// Lazy import so the vi.mock above is hoisted before the module loads
const { CurrencyRatesView } = await import("./CurrencyRatesView");

describe("CurrencyRatesView (FXR-051)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // FXR-051 — loading state: spinner shown while fetch is in flight
  it("renders a loading indicator while getCurrencyPairs is in flight (FXR-051)", async () => {
    let resolveGateway!: (v: unknown) => void;
    vi.mocked(gateway.getCurrencyPairs).mockReturnValue(
      new Promise((r) => {
        resolveGateway = r as typeof resolveGateway;
      }) as ReturnType<typeof gateway.getCurrencyPairs>,
    );

    render(<CurrencyRatesView />);

    expect(screen.getByTestId("currency-rates-loading")).toBeInTheDocument();

    // Clean up the dangling promise
    resolveGateway({ status: "ok", data: [] });
  });

  // FXR-051 — empty state: shown when getCurrencyPairs returns empty list
  it("renders the empty state when no pairs exist (FXR-051)", async () => {
    vi.mocked(gateway.getCurrencyPairs).mockResolvedValue({ status: "ok", data: [] });

    render(<CurrencyRatesView />);

    expect(await screen.findByTestId("currency-rates-empty")).toBeInTheDocument();
  });

  // FXR-051 — error state: DatabaseError from gateway shown as inline error
  it("renders an error message when getCurrencyPairs returns DatabaseError (FXR-051)", async () => {
    vi.mocked(gateway.getCurrencyPairs).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    render(<CurrencyRatesView />);

    expect(await screen.findByTestId("currency-rates-error")).toBeInTheDocument();
  });

  // FXR-051 — pair list: pair row rendered with from/to currencies
  it("renders each pair as a row with from/to currency codes (FXR-051)", async () => {
    vi.mocked(gateway.getCurrencyPairs).mockResolvedValue({
      status: "ok",
      data: [
        {
          from_currency: "USD",
          to_currency: "EUR",
          latest_rate: 920_000,
          latest_rate_date: "2026-06-01",
          latest_rate_source: "Manual",
        },
      ],
    });

    render(<CurrencyRatesView />);

    expect(await screen.findByText("USD")).toBeInTheDocument();
    expect(screen.getByText("EUR")).toBeInTheDocument();
  });

  // FXR-051 — pair with no rate yet shows no rate placeholder
  it("renders a no-rate placeholder for a pair that has never had a rate", async () => {
    vi.mocked(gateway.getCurrencyPairs).mockResolvedValue({
      status: "ok",
      data: [
        {
          from_currency: "GBP",
          to_currency: "USD",
          latest_rate: null,
          latest_rate_date: null,
          latest_rate_source: null,
        },
      ],
    });

    render(<CurrencyRatesView />);

    // Should render the pair but show "—" or similar for rate
    expect(await screen.findByText("GBP")).toBeInTheDocument();
    expect(screen.getByText("USD")).toBeInTheDocument();
    expect(screen.getByTestId("pair-no-rate-GBP-USD")).toBeInTheDocument();
  });

  // FXR-051 — drill-in: clicking a pair row fetches and shows that pair's rates
  it("fetches and shows rates when user drills into a pair (FXR-050/051)", async () => {
    vi.mocked(gateway.getCurrencyPairs).mockResolvedValue({
      status: "ok",
      data: [
        {
          from_currency: "USD",
          to_currency: "EUR",
          latest_rate: 920_000,
          latest_rate_date: "2026-06-01",
          latest_rate_source: "Manual",
        },
      ],
    });
    vi.mocked(gateway.getCurrencyRates).mockResolvedValue({
      status: "ok",
      data: [
        {
          from_currency: "USD",
          to_currency: "EUR",
          date: "2026-06-01",
          rate: 920_000,
          source: "Manual",
        },
      ],
    });

    render(<CurrencyRatesView />);

    const pairRow = await screen.findByTestId("pair-row-USD-EUR");
    await userEvent.click(pairRow);

    expect(gateway.getCurrencyRates).toHaveBeenCalledWith("USD", "EUR");
    expect(await screen.findByTestId("rate-row-USD-EUR-2026-06-01")).toBeInTheDocument();
  });

  // FXR-050 — drill-in rate-load failure surfaces an inline error in the panel (F27)
  it("renders an inline rates error when getCurrencyRates fails on drill-in (FXR-050)", async () => {
    vi.mocked(gateway.getCurrencyPairs).mockResolvedValue({
      status: "ok",
      data: [
        {
          from_currency: "USD",
          to_currency: "EUR",
          latest_rate: 920_000,
          latest_rate_date: "2026-06-01",
          latest_rate_source: "Manual",
        },
      ],
    });
    vi.mocked(gateway.getCurrencyRates).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    render(<CurrencyRatesView />);

    const pairRow = await screen.findByTestId("pair-row-USD-EUR");
    await userEvent.click(pairRow);

    expect(await screen.findByTestId("currency-rates-rates-error")).toBeInTheDocument();
  });

  // FXR-055 — "Add pair" button is present on the pair list
  it("renders an Add Pair button (FXR-054/055)", async () => {
    vi.mocked(gateway.getCurrencyPairs).mockResolvedValue({ status: "ok", data: [] });

    render(<CurrencyRatesView />);

    // Wait for the async load to settle
    await screen.findByTestId("currency-rates-empty");
    expect(screen.getByTestId("action-add-pair")).toBeInTheDocument();
  });
});
