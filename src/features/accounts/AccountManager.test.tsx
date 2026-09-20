import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PriceMovementReport } from "@/bindings";
import { AccountManager } from "./AccountManager";

const { mockNavigate, mockUsePriceMovementReport, mockStoreState } = vi.hoisted(() => ({
  mockNavigate: vi.fn(),
  mockUsePriceMovementReport: vi.fn(),
  mockStoreState: { unpricedAssets: [] as unknown[] },
}));

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

// PMV-018 — the manager asks the store's own selector whether the unupdated-prices
// modal (MKT-172) holds the screen; the selector is the real one, only the state is faked.
// Loading the real module is safe because src/test-setup.ts mocks @tauri-apps/api/core,
// which every top-level import of store.ts bottoms out in.
vi.mock("@/lib/store", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/store")>();
  return {
    selectUnpricedModalOpen: actual.selectUnpricedModalOpen,
    useAppStore: (selector: (state: typeof mockStoreState) => unknown) => selector(mockStoreState),
  };
});

// Stub the children so the manager renders in isolation (no gateway, no Tauri).
vi.mock("./account_table/AccountTable", () => ({
  AccountTable: () => <div data-testid="account-table" />,
}));
vi.mock("./add_account/AddAccountModal", () => ({ AddAccountModal: () => null }));
vi.mock("./refresh_prices/useRefreshGlobalPrices", () => ({
  useRefreshGlobalPrices: () => ({ isPending: false, refresh: vi.fn() }),
}));

// PMV-013/016/061 — the dialog and its mount-scoped hook are covered on their
// own (PriceMovementDialog.test.tsx, usePriceMovementReport.test.ts); here we
// only verify AccountManager wires the hook's output into the dialog. The stub
// honours `isOpen` so the PMV-018 ordering is observable.
vi.mock("./price_movement/usePriceMovementReport", () => ({
  usePriceMovementReport: () => mockUsePriceMovementReport(),
}));
vi.mock("./price_movement/PriceMovementDialog", () => ({
  PriceMovementDialog: (props: { report: unknown; isOpen: boolean; onDismiss: () => void }) =>
    props.isOpen ? (
      <button type="button" data-testid="price-movement-dialog-stub" onClick={props.onDismiss} />
    ) : null,
}));

describe("AccountManager — global performance entry point (GPF)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockStoreState.unpricedAssets = [];
    mockUsePriceMovementReport.mockReturnValue({ report: null, dismiss: vi.fn() });
  });

  it("renders the performance entry button next to the search field", () => {
    render(<AccountManager />);
    expect(document.querySelector("#accounts-performance")).toBeInTheDocument();
  });

  it("navigates to /performance when the entry button is clicked", () => {
    render(<AccountManager />);
    fireEvent.click(document.querySelector("#accounts-performance")!);
    expect(mockNavigate).toHaveBeenCalledWith({ to: "/performance" });
  });
});

// PMV-013 — AccountManager mounts usePriceMovementReport and opens the dialog
// whenever a report is present and nothing else is holding the screen. Both
// the hook and the dialog are mocked here — their own behavior is covered in
// usePriceMovementReport.test.ts and PriceMovementDialog.test.tsx.
describe("AccountManager — price movement dialog (PMV-013/018)", () => {
  const makeReport = (): PriceMovementReport => ({
    rows: [],
    total_before: 0,
    total_after: 0,
    total_currency: "EUR",
    total_movement_pct: null,
    total_movement_amount: null,
    observed_from: null,
    observed_to: null,
    incomplete: false,
  });

  beforeEach(() => {
    vi.clearAllMocks();
    mockStoreState.unpricedAssets = [];
    mockUsePriceMovementReport.mockReturnValue({ report: null, dismiss: vi.fn() });
  });

  it("does not render the dialog when the hook has no report", () => {
    render(<AccountManager />);

    expect(screen.queryByTestId("price-movement-dialog-stub")).not.toBeInTheDocument();
  });

  // PMV-013 — the dialog is a surface of its own; the account rows stay mounted
  // behind it rather than being replaced by it.
  it("opens the dialog over the account rows when the hook has a report", () => {
    mockUsePriceMovementReport.mockReturnValue({ report: makeReport(), dismiss: vi.fn() });

    render(<AccountManager />);

    expect(screen.getByTestId("price-movement-dialog-stub")).toBeInTheDocument();
    expect(screen.getByTestId("account-table")).toBeInTheDocument();
  });

  // PMV-018 — the unupdated-prices modal asks the user for something, so the
  // report waits for it instead of stacking over it.
  it("keeps the dialog closed while the unupdated-prices modal is open", () => {
    mockUsePriceMovementReport.mockReturnValue({ report: makeReport(), dismiss: vi.fn() });
    mockStoreState.unpricedAssets = [{ asset_id: "a1" }];

    render(<AccountManager />);

    expect(screen.queryByTestId("price-movement-dialog-stub")).not.toBeInTheDocument();
  });

  // PMV-018 — and it opens once that modal has cleared, rather than being lost.
  it("opens the dialog once the unupdated-prices modal has cleared", () => {
    mockUsePriceMovementReport.mockReturnValue({ report: makeReport(), dismiss: vi.fn() });
    mockStoreState.unpricedAssets = [{ asset_id: "a1" }];
    const { rerender } = render(<AccountManager />);
    expect(screen.queryByTestId("price-movement-dialog-stub")).not.toBeInTheDocument();

    mockStoreState.unpricedAssets = [];
    rerender(<AccountManager />);

    expect(screen.getByTestId("price-movement-dialog-stub")).toBeInTheDocument();
  });

  // PMV-061 — dismissing routes through the hook, which is the only place
  // that decides the report is gone for good.
  it("wires the hook's dismiss into the dialog", () => {
    const dismiss = vi.fn();
    mockUsePriceMovementReport.mockReturnValue({ report: makeReport(), dismiss });

    render(<AccountManager />);
    fireEvent.click(screen.getByTestId("price-movement-dialog-stub"));

    expect(dismiss).toHaveBeenCalledTimes(1);
  });
});
