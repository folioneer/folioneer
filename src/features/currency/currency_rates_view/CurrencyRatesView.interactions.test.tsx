import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CurrencyPairSummary, CurrencyRate } from "@/bindings";
import * as viewHook from "./useCurrencyRatesView";

// Stub child modals so the view's open/close/success wiring is testable in
// isolation (mock the *View hook + stub child modals — the view-harness pattern).
vi.mock("../declare_pair/DeclarePairModal", () => ({
  DeclarePairModal: ({
    isOpen,
    onClose,
    onSuccess,
  }: {
    isOpen: boolean;
    onClose: () => void;
    onSuccess: () => void;
  }) =>
    isOpen ? (
      <div data-testid="mock-declare-pair">
        <button type="button" data-testid="declare-close" onClick={onClose}>
          close
        </button>
        <button type="button" data-testid="declare-success" onClick={onSuccess}>
          success
        </button>
      </div>
    ) : null,
}));

vi.mock("../record_rate/RecordRateModal", () => ({
  RecordRateModal: ({
    isOpen,
    initialRate,
    onClose,
    onSuccess,
  }: {
    isOpen: boolean;
    initialRate?: CurrencyRate;
    onClose: () => void;
    onSuccess: () => void;
  }) =>
    isOpen ? (
      <div data-testid={initialRate ? "mock-edit-rate" : "mock-record-rate"}>
        <button type="button" data-testid="record-close" onClick={onClose}>
          close
        </button>
        <button type="button" data-testid="record-success" onClick={onSuccess}>
          success
        </button>
      </div>
    ) : null,
  DeleteRateConfirmation: ({
    isOpen,
    onClose,
    onSuccess,
  }: {
    isOpen: boolean;
    onClose: () => void;
    onSuccess: () => void;
  }) =>
    isOpen ? (
      <div data-testid="mock-delete-rate">
        <button type="button" data-testid="delete-close" onClick={onClose}>
          close
        </button>
        <button type="button" data-testid="delete-success" onClick={onSuccess}>
          success
        </button>
      </div>
    ) : null,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en" },
  }),
}));

const { CurrencyRatesView } = await import("./CurrencyRatesView");

const PAIR: CurrencyPairSummary = {
  from_currency: "USD",
  to_currency: "EUR",
  latest_rate: 920_000,
  latest_rate_date: "2026-06-01",
  latest_rate_source: "Manual",
};
const RATE: CurrencyRate = {
  from_currency: "USD",
  to_currency: "EUR",
  date: "2026-06-01",
  rate: 920_000,
  source: "Manual",
};

const baseHook = {
  isLoading: false,
  error: null,
  pairs: [PAIR],
  selectedPair: null as { fromCurrency: string; toCurrency: string } | null,
  rates: [] as CurrencyRate[],
  ratesError: null,
  selectPair: vi.fn(),
  clearSelection: vi.fn(),
  refetch: vi.fn(),
  isBackfilling: false,
  backfillHistory: vi.fn().mockResolvedValue({ status: "ok" as const, ratesWritten: 0 }),
};

function mockHook(overrides: Partial<typeof baseHook> = {}) {
  vi.spyOn(viewHook, "useCurrencyRatesView").mockReturnValue({ ...baseHook, ...overrides });
}

describe("CurrencyRatesView interactions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // FXR-054 — the Add-pair action opens the declare-pair modal
  it("opens the declare-pair modal and refetches on success", async () => {
    const user = userEvent.setup();
    const refetch = vi.fn();
    mockHook({ refetch });
    render(<CurrencyRatesView />);

    await user.click(screen.getByTestId("action-add-pair"));
    expect(screen.getByTestId("mock-declare-pair")).toBeInTheDocument();

    await user.click(screen.getByTestId("declare-success"));
    expect(refetch).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId("mock-declare-pair")).not.toBeInTheDocument();
  });

  // FXR-055 — closing the declare-pair modal does not refetch
  it("closes the declare-pair modal without refetching", async () => {
    const user = userEvent.setup();
    const refetch = vi.fn();
    mockHook({ refetch });
    render(<CurrencyRatesView />);

    await user.click(screen.getByTestId("action-add-pair"));
    await user.click(screen.getByTestId("declare-close"));
    expect(screen.queryByTestId("mock-declare-pair")).not.toBeInTheDocument();
    expect(refetch).not.toHaveBeenCalled();
  });

  // FXR-051 — clicking a pair row drills in
  it("selects a pair when its row is clicked", async () => {
    const user = userEvent.setup();
    const selectPair = vi.fn();
    mockHook({ selectPair });
    render(<CurrencyRatesView />);

    await user.click(screen.getByTestId("pair-row-USD-EUR"));
    expect(selectPair).toHaveBeenCalledWith("USD", "EUR");
  });

  // FXR-051 — the pair row is keyboard-accessible (Enter drills in)
  it("selects a pair when Enter is pressed on its row", async () => {
    const user = userEvent.setup();
    const selectPair = vi.fn();
    mockHook({ selectPair });
    render(<CurrencyRatesView />);

    screen.getByTestId("pair-row-USD-EUR").focus();
    await user.keyboard("{Enter}");
    expect(selectPair).toHaveBeenCalledWith("USD", "EUR");
  });

  // FXR-050 — the drill-in close button clears the selection
  it("clears the selection from the drill-in header", async () => {
    const user = userEvent.setup();
    const clearSelection = vi.fn();
    mockHook({
      selectedPair: { fromCurrency: "USD", toCurrency: "EUR" },
      rates: [RATE],
      clearSelection,
    });
    render(<CurrencyRatesView />);

    await user.click(screen.getByText("action.close"));
    expect(clearSelection).toHaveBeenCalledTimes(1);
  });

  // FXR-020 — the record-rate action opens the record modal (create mode)
  it("opens and closes the record-rate modal from the drill-in", async () => {
    const user = userEvent.setup();
    mockHook({ selectedPair: { fromCurrency: "USD", toCurrency: "EUR" }, rates: [RATE] });
    render(<CurrencyRatesView />);

    await user.click(screen.getByTestId("action-record-rate"));
    expect(screen.getByTestId("mock-record-rate")).toBeInTheDocument();

    await user.click(screen.getByTestId("record-success"));
    expect(screen.queryByTestId("mock-record-rate")).not.toBeInTheDocument();
  });

  // FXR-052 — editing a rate opens the record modal in edit mode
  it("opens the record modal in edit mode for a rate row", async () => {
    const user = userEvent.setup();
    mockHook({ selectedPair: { fromCurrency: "USD", toCurrency: "EUR" }, rates: [RATE] });
    render(<CurrencyRatesView />);

    await user.click(screen.getByText("action.edit"));
    expect(screen.getByTestId("mock-edit-rate")).toBeInTheDocument();

    await user.click(screen.getByTestId("record-close"));
    expect(screen.queryByTestId("mock-edit-rate")).not.toBeInTheDocument();
  });

  // FXR-053 — deleting a rate opens the delete confirmation
  it("opens and confirms the delete-rate dialog for a rate row", async () => {
    const user = userEvent.setup();
    mockHook({ selectedPair: { fromCurrency: "USD", toCurrency: "EUR" }, rates: [RATE] });
    render(<CurrencyRatesView />);

    await user.click(screen.getByText("action.delete"));
    expect(screen.getByTestId("mock-delete-rate")).toBeInTheDocument();

    await user.click(screen.getByTestId("delete-success"));
    expect(screen.queryByTestId("mock-delete-rate")).not.toBeInTheDocument();
  });

  // FXR-110 — the history download triggers the backfill action
  it("triggers the history backfill from its header action", async () => {
    const user = userEvent.setup();
    const backfillHistory = vi.fn().mockResolvedValue({ status: "ok" as const, ratesWritten: 42 });
    mockHook({ backfillHistory });
    render(<CurrencyRatesView />);

    await user.click(screen.getByTestId("action-backfill-history"));
    expect(backfillHistory).toHaveBeenCalledTimes(1);
  });

  // FXR-110 — the action is disabled while the download is in flight
  it("disables the history backfill action while it runs", () => {
    mockHook({ isBackfilling: true });
    render(<CurrencyRatesView />);

    expect(screen.getByTestId("action-backfill-history")).toBeDisabled();
  });
});
