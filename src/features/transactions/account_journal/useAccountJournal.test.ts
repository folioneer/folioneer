import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, Asset, Transaction } from "@/bindings";
import { microToFormatted } from "@/lib/microUnits";
import { useAppStore } from "@/lib/store";
import { useAccountJournal } from "./useAccountJournal";

vi.mock("@tanstack/react-router", () => ({
  useParams: () => ({ accountId: "account-1" }),
}));

const mockGetAll = vi.fn();
const mockSubscribe = vi.fn();

vi.mock("../gateway", () => ({
  transactionGateway: {
    getAllTransactionsForAccount: (...args: unknown[]) => mockGetAll(...args),
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
    mockGetAll.mockResolvedValue({
      status: "ok",
      data: [
        tx({ id: "a", asset_id: "asset-1", date: "2024-01-01", total_amount: 100 * MICRO }),
        tx({
          id: "b",
          asset_id: "asset-2",
          date: "2024-03-01",
          transaction_type: "Sell",
          total_amount: 500 * MICRO,
        }),
        tx({ id: "c", asset_id: "asset-1", date: "2024-02-01", total_amount: 300 * MICRO }),
      ],
    });
    mockSubscribe.mockImplementation(async (cb: (type: string) => void) => {
      eventCallback = cb;
      return () => {};
    });
  });

  it("loads all account transactions, sorted latest-first by default", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    expect(mockGetAll).toHaveBeenCalledWith("account-1");
    expect(result.current.filteredSortedRows.map((r) => r.id)).toEqual(["b", "c", "a"]);
  });

  it("orders same-date events by created_at, applying the sort direction (regression)", async () => {
    // All on the same day, fed in created_at order (as the backend returns them).
    mockGetAll.mockResolvedValue({
      status: "ok",
      data: [
        tx({
          id: "A",
          date: "2024-05-01",
          transaction_type: "Deposit",
          total_amount: 1000 * MICRO,
          created_at: "2024-05-01T09:00:00.000Z",
        }),
        tx({
          id: "B",
          date: "2024-05-01",
          transaction_type: "Purchase",
          total_amount: 300 * MICRO,
          created_at: "2024-05-01T10:00:00.000Z",
        }),
        tx({
          id: "C",
          date: "2024-05-01",
          transaction_type: "Sell",
          total_amount: 200 * MICRO,
          created_at: "2024-05-01T11:00:00.000Z",
        }),
      ],
    });
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});

    // desc (default): newest-created first — NOT the input order (the bug showed
    // input order here, flipping the balance column at the day boundary).
    expect(result.current.filteredSortedRows.map((r) => r.id)).toEqual(["C", "B", "A"]);

    // Each row still carries its true post-event balance from the chronological
    // replay (A 1000 → B 700 → C 900), independent of display order.
    const byId = Object.fromEntries(
      result.current.filteredSortedRows.map((r) => [r.id, r.balance]),
    );
    expect(byId.A).toBe(microToFormatted(1000 * MICRO));
    expect(byId.B).toBe(microToFormatted(700 * MICRO));
    expect(byId.C).toBe(microToFormatted(900 * MICRO));

    // asc: oldest-created first.
    act(() => result.current.toggleSortDirection());
    expect(result.current.filteredSortedRows.map((r) => r.id)).toEqual(["A", "B", "C"]);
  });

  it("filters by asset", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    act(() => result.current.setFilter("assetId", "asset-1"));
    expect(result.current.filteredSortedRows.map((r) => r.id)).toEqual(["c", "a"]);
  });

  it("filters by transaction type", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    act(() => result.current.setFilter("type", "Sell"));
    expect(result.current.filteredSortedRows.map((r) => r.id)).toEqual(["b"]);
  });

  it("filters by amount range (inclusive)", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    act(() => {
      result.current.setFilter("amountMin", "200");
      result.current.setFilter("amountMax", "400");
    });
    expect(result.current.filteredSortedRows.map((r) => r.id)).toEqual(["c"]);
  });

  it("clears all filters", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    act(() => result.current.setFilter("type", "Sell"));
    expect(result.current.filteredSortedRows).toHaveLength(1);
    act(() => result.current.clearFilters());
    expect(result.current.filteredSortedRows).toHaveLength(3);
  });

  it("re-fetches on a TransactionUpdated event", async () => {
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    expect(mockGetAll).toHaveBeenCalledTimes(1);
    await act(async () => {
      eventCallback?.("TransactionUpdated");
    });
    expect(mockGetAll).toHaveBeenCalledTimes(2);
    // unrelated event is ignored
    await act(async () => {
      eventCallback?.("AssetUpdated");
    });
    expect(mockGetAll).toHaveBeenCalledTimes(2);
    expect(result.current.hasTransactions).toBe(true);
  });

  // #020 / SYN-064 — changes applied from another device arrive as one SyncCompleted.
  it("re-fetches once when a sync completes", async () => {
    renderHook(() => useAccountJournal());
    await act(async () => {});
    expect(mockGetAll).toHaveBeenCalledTimes(1);
    await act(async () => {
      eventCallback?.("SyncCompleted");
    });
    expect(mockGetAll).toHaveBeenCalledTimes(2);
  });

  it("computes bank-statement cash columns + running balance over the full set", async () => {
    mockGetAll.mockResolvedValue({
      status: "ok",
      data: [
        tx({
          id: "dep",
          date: "2024-01-01",
          transaction_type: "Deposit",
          total_amount: 1000 * MICRO,
        }),
        tx({
          id: "buy",
          date: "2024-02-01",
          transaction_type: "Purchase",
          total_amount: 300 * MICRO,
        }),
        tx({
          id: "div",
          date: "2024-03-01",
          transaction_type: "Dividend",
          total_amount: 50 * MICRO,
        }),
        tx({ id: "sell", date: "2024-04-01", transaction_type: "Sell", total_amount: 200 * MICRO }),
      ],
    });
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    const byId = Object.fromEntries(result.current.filteredSortedRows.map((r) => [r.id, r]));

    // Deposit/Dividend/Sell are credits (cash in); Purchase is a debit (cash out).
    expect(byId.dep?.cashIn).toBe(microToFormatted(1000 * MICRO));
    expect(byId.dep?.cashOut).toBe("");
    expect(byId.buy?.cashOut).toBe(microToFormatted(300 * MICRO));
    expect(byId.buy?.cashIn).toBe("");

    // Running balance: 1000 → 700 → 750 → 950.
    expect(byId.dep?.balance).toBe(microToFormatted(1000 * MICRO));
    expect(byId.buy?.balance).toBe(microToFormatted(700 * MICRO));
    expect(byId.div?.balance).toBe(microToFormatted(750 * MICRO));
    expect(byId.sell?.balance).toBe(microToFormatted(950 * MICRO));
  });

  it("leaves cash columns blank for a non-cash type and keeps the balance flat", async () => {
    mockGetAll.mockResolvedValue({
      status: "ok",
      data: [
        tx({
          id: "ob",
          date: "2024-01-01",
          transaction_type: "OpeningBalance",
          total_amount: 500 * MICRO,
        }),
      ],
    });
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    const row = result.current.filteredSortedRows[0];
    expect(row?.cashOut).toBe("");
    expect(row?.cashIn).toBe("");
    expect(row?.balance).toBe(microToFormatted(0));
  });

  it("keeps the true full-history balance on a filtered row", async () => {
    mockGetAll.mockResolvedValue({
      status: "ok",
      data: [
        tx({
          id: "dep",
          date: "2024-01-01",
          transaction_type: "Deposit",
          total_amount: 1000 * MICRO,
        }),
        tx({
          id: "buy",
          asset_id: "asset-1",
          date: "2024-02-01",
          transaction_type: "Purchase",
          total_amount: 300 * MICRO,
        }),
      ],
    });
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    act(() => result.current.setFilter("type", "Purchase"));
    // Only the Purchase row is visible, but its balance still reflects the prior Deposit.
    expect(result.current.filteredSortedRows.map((r) => r.id)).toEqual(["buy"]);
    expect(result.current.filteredSortedRows[0]?.balance).toBe(microToFormatted(700 * MICRO));
  });

  it("surfaces an i18n error when the load fails", async () => {
    mockGetAll.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });
    const { result } = renderHook(() => useAccountJournal());
    await act(async () => {});
    expect(result.current.error).not.toBeNull();
    expect(result.current.hasTransactions).toBe(false);
  });
});
