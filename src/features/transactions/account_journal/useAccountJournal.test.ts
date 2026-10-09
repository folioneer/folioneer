import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, AccountJournal, Asset, Transaction } from "@/bindings";
import { microToFormatted } from "@/lib/microUnits";
import { useAppStore } from "@/lib/store";
import { useAccountJournal } from "./useAccountJournal";

vi.mock("@tanstack/react-router", () => ({
  useParams: () => ({ accountId: "account-1" }),
}));

const mockGetJournal = vi.fn();
const mockSubscribe = vi.fn();

vi.mock("../gateway", () => ({
  transactionGateway: {
    getAccountJournal: (...args: unknown[]) => mockGetJournal(...args),
    subscribeToEvents: (...args: unknown[]) => mockSubscribe(...args),
  },
}));

vi.mock("@/lib/logger", () => ({ logger: { error: vi.fn() } }));

const MICRO = 1_000_000;

const tx = (over: Partial<Transaction>): Transaction =>
  ({
    id: "tx",
    account_id: "account-1",
    asset_id: "asset-1",
    transaction_type: "Purchase",
    date: "2024-01-01",
    quantity: MICRO,
    unit_price: MICRO,
    exchange_rate: MICRO,
    fees: 0,
    total_amount: 100 * MICRO,
    note: null,
    realized_pnl: null,
    created_at: "2024-01-01T00:00:00.000001Z",
    ...over,
  }) as Transaction;

// What the core answers (TXL-060): rows in the order asked, cash columns computed.
const JOURNAL: AccountJournal = {
  rows: [
    {
      transaction: tx({
        id: "b",
        asset_id: "asset-2",
        transaction_type: "Sell",
        date: "2024-03-01",
      }),
      cash_out: null,
      cash_in: 500 * MICRO,
      cash_balance: 900 * MICRO,
      recorded_by: {
        agent: "Claude Code",
        session: "session-1",
        session_started_at: "2026-10-09T14:32:00+02:00",
      },
    },
    {
      transaction: tx({ id: "a", asset_id: "asset-1", date: "2024-01-01" }),
      cash_out: 100 * MICRO,
      cash_in: null,
      cash_balance: 400 * MICRO,
      recorded_by: null,
    },
  ],
  asset_ids: ["asset-1", "asset-2"],
  transaction_types: ["Purchase", "Sell"],
  has_transactions: true,
};

const NO_FILTER = {
  asset_id: null,
  transaction_type: null,
  amount_min: null,
  amount_max: null,
  newest_first: true,
};

describe("useAccountJournal", () => {
  let eventCallback: ((type: string) => void) | undefined;

  beforeEach(() => {
    vi.clearAllMocks();
    eventCallback = undefined;
    useAppStore.setState({
      assets: [
        { id: "asset-1", name: "Apple" },
        { id: "asset-2", name: "Google" },
      ] as Asset[],
      accounts: [{ id: "account-1", name: "My Account" }] as Account[],
    });
    mockGetJournal.mockResolvedValue({ status: "ok", data: JOURNAL });
    mockSubscribe.mockImplementation(async (cb: (type: string) => void) => {
      eventCallback = cb;
      return () => {};
    });
  });

  // TXL-061 — the journal loads newest first with no filter, rows in the order returned
  it("loads the account journal newest first and shows its rows as returned", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});

    expect(mockGetJournal).toHaveBeenCalledWith("account-1", NO_FILTER);
    expect(result.current.filteredSortedRows.map((r) => r.id)).toEqual(["b", "a"]);
    expect(result.current.hasTransactions).toBe(true);
  });

  // TXL-061 — each row shows the cash columns the core computed
  it("shows the cash columns the core computed", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});

    const [sale, purchase] = result.current.filteredSortedRows;
    expect(sale).toEqual(
      expect.objectContaining({
        cashIn: microToFormatted(500 * MICRO),
        cashOut: "",
        balance: microToFormatted(900 * MICRO),
      }),
    );
    expect(purchase).toEqual(
      expect.objectContaining({ cashOut: microToFormatted(100 * MICRO), cashIn: "" }),
    );
  });

  // AGT-045 — a row an agent recorded carries its agent and session; the owner's own does not
  it("shows which agent session recorded a row", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});

    const [sale, purchase] = result.current.filteredSortedRows;
    expect(sale?.recordedBy).toEqual({
      agent: "Claude Code",
      sessionStartedAt: "2026-10-09T14:32:00+02:00",
    });
    expect(purchase?.recordedBy).toBeNull();
  });

  // TXL-061 — the filter choices are the assets and types the journal lists
  it("offers the assets and types the journal lists", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});

    expect(result.current.assetFilterOptions).toEqual([
      { value: "asset-1", label: "Apple" },
      { value: "asset-2", label: "Google" },
    ]);
    expect(result.current.typeFilterOptions.map((o) => o.value)).toEqual(["Purchase", "Sell"]);
  });

  // TXL-060 / TXL-061 — the chosen filters and order are sent to the core, amounts in micro-units
  it("sends the chosen filters and order to the core", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});

    await act(async () => {
      result.current.setFilter("assetId", "asset-1");
      result.current.setFilter("type", "Purchase");
      result.current.setFilter("amountMin", "100");
      result.current.setFilter("amountMax", "300.5");
      result.current.toggleSortDirection();
    });

    expect(mockGetJournal).toHaveBeenLastCalledWith("account-1", {
      asset_id: "asset-1",
      transaction_type: "Purchase",
      amount_min: 100 * MICRO,
      amount_max: 300_500_000,
      newest_first: false,
    });

    await act(async () => {
      result.current.clearFilters();
      result.current.toggleSortDirection();
    });
    expect(mockGetJournal).toHaveBeenLastCalledWith("account-1", NO_FILTER);
  });

  // TXL-061 — only the answer to the latest filters is shown
  it("drops an answer to earlier filters that arrives late", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    let answerEarlier: (value: unknown) => void = () => {};
    mockGetJournal
      .mockImplementationOnce(() => new Promise((resolve) => (answerEarlier = resolve)))
      .mockResolvedValueOnce({
        status: "ok",
        data: { ...JOURNAL, rows: [JOURNAL.rows[1]] },
      });

    await act(async () => {
      result.current.setFilter("type", "Sell");
    });
    await act(async () => {
      result.current.setFilter("type", "Purchase");
    });
    await act(async () => {
      answerEarlier({ status: "ok", data: JOURNAL });
    });

    expect(result.current.filteredSortedRows.map((r) => r.id)).toEqual(["a"]);
  });

  it("re-fetches on a TransactionUpdated event", async () => {
    renderHook(() => useAccountJournal());
    await act(async () => {});
    expect(mockGetJournal).toHaveBeenCalledTimes(1);
    await act(async () => {
      eventCallback?.("TransactionUpdated");
    });
    expect(mockGetJournal).toHaveBeenCalledTimes(2);
    await act(async () => {
      eventCallback?.("AssetUpdated");
    });
    expect(mockGetJournal).toHaveBeenCalledTimes(2);
  });

  it("re-fetches once when a sync completes", async () => {
    renderHook(() => useAccountJournal());
    await act(async () => {});
    await act(async () => {
      eventCallback?.("SyncCompleted");
    });
    expect(mockGetJournal).toHaveBeenCalledTimes(2);
  });

  it("surfaces an i18n error when the load fails", async () => {
    mockGetJournal.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    expect(result.current.error).not.toBeNull();
    expect(result.current.hasTransactions).toBe(false);
  });

  // F29 — a change re-fetches the journal without a loading state: the rows stay mounted.
  it("keeps the journal on screen while a change re-fetches it", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    expect(result.current.isLoading).toBe(false);
    const shown = result.current.filteredSortedRows.length;
    mockGetJournal.mockReturnValue(new Promise(() => {}));

    act(() => {
      eventCallback?.("TransactionUpdated");
    });

    expect(result.current.isLoading).toBe(false);
    expect(result.current.filteredSortedRows.length).toBe(shown);
  });

  // F29 — a retry the user asks for (the Retry button) shows the loading state.
  it("shows the loading state while the user retries", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    expect(result.current.isLoading).toBe(false);
    mockGetJournal.mockReturnValue(new Promise(() => {}));

    act(() => {
      void result.current.reload();
    });

    expect(result.current.isLoading).toBe(true);
  });

  // F29 — the refresh after a delete or an edit keeps the rows on screen.
  it("keeps the journal on screen while it refreshes after a change", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    expect(result.current.isLoading).toBe(false);
    mockGetJournal.mockReturnValue(new Promise(() => {}));

    act(() => {
      void result.current.refresh();
    });

    expect(result.current.isLoading).toBe(false);
  });

  // A thrown load shows the generic error
  it("shows the generic error when the load throws", async () => {
    mockGetJournal.mockRejectedValue(new Error("down"));
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    expect(result.current.error).toEqual({ key: "error.Unknown" });
  });
});
