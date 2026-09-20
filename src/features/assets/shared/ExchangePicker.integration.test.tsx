import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Exchange } from "@/bindings";

// Mock react-i18next to return the key so assertions use i18n keys (F24)
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

// Mock the assets store so tests control supportedExchanges without IPC (F3)
const mockLoadSupportedExchanges = vi.fn();

vi.mock("../store", () => ({
  useAssetsStore: (
    selector: (s: {
      supportedExchanges: Exchange[];
      loadSupportedExchanges: () => void;
    }) => unknown,
  ) =>
    selector({
      supportedExchanges: STORE_EXCHANGES,
      loadSupportedExchanges: mockLoadSupportedExchanges,
    }),
}));

// Module-level mutable — tests override this before rendering
let STORE_EXCHANGES: Exchange[] = [];

const SAMPLE_EXCHANGES: Exchange[] = [
  { code: "XPAR", label: "Euronext Paris" },
  { code: "XNAS", label: "NASDAQ" },
];

// Dynamic import after mocks so the component picks up mocked store + i18n
const { ExchangePicker } = await import("./ExchangePicker");

describe("ExchangePicker", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    STORE_EXCHANGES = SAMPLE_EXCHANGES;
  });

  // AST-021 — picker renders all supported exchanges and the (none) option
  it("renders all supported exchanges from the store plus the none option", () => {
    render(<ExchangePicker value={null} onChange={vi.fn()} />);

    // (none) option — i18n key per F24
    expect(screen.getByRole("option", { name: "asset.exchange_none_option" })).toBeInTheDocument();
    // Exchange options formatted as "{label} ({code})"
    expect(screen.getByRole("option", { name: "Euronext Paris (XPAR)" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "NASDAQ (XNAS)" })).toBeInTheDocument();
  });

  // AST-021 — selecting an exchange fires onChange with the Exchange object
  it("calls onChange with the Exchange object when a venue is selected", async () => {
    const onChange = vi.fn();
    render(<ExchangePicker value={null} onChange={onChange} />);

    await userEvent.selectOptions(screen.getByTestId("asset-exchange-picker"), "XPAR");

    expect(onChange).toHaveBeenCalledWith({ code: "XPAR", label: "Euronext Paris" });
  });

  // AST-021 — selecting (none) fires onChange(null)
  it("calls onChange(null) when the none option is selected", async () => {
    const onChange = vi.fn();
    render(
      <ExchangePicker value={{ code: "XPAR", label: "Euronext Paris" }} onChange={onChange} />,
    );

    await userEvent.selectOptions(screen.getByTestId("asset-exchange-picker"), "");

    expect(onChange).toHaveBeenCalledWith(null);
  });

  // AST-021 — current value is reflected as the selected option
  it("reflects the current value as the selected option", () => {
    render(<ExchangePicker value={{ code: "XNAS", label: "NASDAQ" }} onChange={vi.fn()} />);

    const select = screen.getByTestId("asset-exchange-picker") as HTMLSelectElement;
    expect(select.value).toBe("XNAS");
  });

  // AST-021 — null value reflects the (none) option as selected
  it("reflects null value as the none option being selected", () => {
    render(<ExchangePicker value={null} onChange={vi.fn()} />);

    const select = screen.getByTestId("asset-exchange-picker") as HTMLSelectElement;
    expect(select.value).toBe("");
  });

  // Session-static cache — mounting with empty store triggers loadSupportedExchanges
  it("calls loadSupportedExchanges on mount when store is empty", () => {
    STORE_EXCHANGES = [];
    render(<ExchangePicker value={null} onChange={vi.fn()} />);
    expect(mockLoadSupportedExchanges).toHaveBeenCalledTimes(1);
  });

  // Session-static cache — does not trigger load when store is already populated
  it("does not call loadSupportedExchanges when store is already populated", () => {
    STORE_EXCHANGES = SAMPLE_EXCHANGES;
    render(<ExchangePicker value={null} onChange={vi.fn()} />);
    expect(mockLoadSupportedExchanges).not.toHaveBeenCalled();
  });

  // F25 — label uses i18n key, not a literal string (F24)
  it("renders the field label using the asset.field_exchange i18n key", () => {
    render(<ExchangePicker value={null} onChange={vi.fn()} />);
    expect(screen.getByText("asset.field_exchange")).toBeInTheDocument();
  });
});
