import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, Asset } from "@/bindings";
import { useAppStore } from "@/lib/store";
import { useTransactionList } from "./useTransactionList";

const mockNavigate = vi.fn();

vi.mock("@tanstack/react-router", () => ({
  useParams: () => ({ accountId: "account-1", assetId: "asset-1" }),
  useNavigate: () => mockNavigate,
}));

const mockGetAssetIdsForAccount = vi.fn();
const mockGetTransactions = vi.fn();
const mockGetJournal = vi.fn();
const mockSubscribeToEvents = vi.fn();

// The core's account journal filtered to the asset (TXL-060), standing in for the backend:
// `mockGetTransactions` holds the asset's transactions oldest first, and the journal
// returns them in the order asked.
const journalOf = async (
  accountId: string,
  filter: { asset_id: string; newest_first: boolean },
) => {
  mockGetJournal(accountId, filter);
  const res = await mockGetTransactions(accountId, filter.asset_id);
  if (res.status !== "ok") return res;
  const ordered = filter.newest_first ? [...res.data].reverse() : res.data;
  return {
    status: "ok",
    data: {
      rows: ordered.map((transaction: unknown) => ({
        transaction,
        cash_out: null,
        cash_in: null,
        cash_balance: 0,
      })),
      asset_ids: [],
      transaction_types: [],
      has_transactions: ordered.length > 0,
    },
  };
};

vi.mock("../gateway", () => ({
  transactionGateway: {
    getAssetIdsForAccount: (...args: unknown[]) => mockGetAssetIdsForAccount(...args),
    getAccountJournal: (accountId: string, filter: { asset_id: string; newest_first: boolean }) =>
      journalOf(accountId, filter),
    subscribeToEvents: (...args: unknown[]) => mockSubscribeToEvents(...args),
  },
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn() },
}));

const MICRO = 1_000_000;

const makeTx = (id: string, date: string) => ({
  id,
  account_id: "account-1",
  asset_id: "asset-1",
  transaction_type: "Purchase",
  date,
  quantity: MICRO,
  unit_price: MICRO,
  exchange_rate: MICRO,
  fees: 0,
  total_amount: MICRO,
  note: null,
});

describe("useTransactionList", () => {
  let eventCallback: ((type: string) => void) | undefined;

  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({
      assets: [
        { id: "asset-1", name: "Apple" },
        { id: "asset-2", name: "Google" },
      ] as Asset[],
      accounts: [
        { id: "account-1", name: "My Account" },
        { id: "account-2", name: "Other Account" },
      ] as Account[],
    });
    mockGetAssetIdsForAccount.mockResolvedValue({
      status: "ok",
      data: ["asset-1", "asset-2"],
    });
    mockGetTransactions.mockResolvedValue({
      status: "ok",
      data: [makeTx("tx-1", "2024-01-01"), makeTx("tx-2", "2024-03-01")],
    });
    eventCallback = undefined;
    mockSubscribeToEvents.mockImplementation(async (cb: (type: string) => void) => {
      eventCallback = cb;
      return () => {};
    });
  });

  // On mount: fetches asset IDs and transactions from route params (TXL-011)
  it("fetches asset IDs and transactions on mount", async () => {
    const { result } = renderHook(() => useTransactionList());
    await act(async () => {});
    expect(mockGetAssetIdsForAccount).toHaveBeenCalledWith("account-1");
    expect(mockGetTransactions).toHaveBeenCalledWith("account-1", "asset-1");
    expect(result.current.assetOptions).toHaveLength(2);
    expect(result.current.transactions).toHaveLength(2);
  });

  // Default sort is descending — most recent first (TXL-024)
  it("sorts transactions descending by default", async () => {
    const { result } = renderHook(() => useTransactionList());
    await act(async () => {});
    const rows = result.current.sortedTransactions;
    expect(rows.at(0)?.date).toBe("2024-03-01");
    expect(rows.at(1)?.date).toBe("2024-01-01");
  });

  // TXL-053 / TXL-024 — a retry after a failed load keeps the order the user chose
  it("retries in the order the user chose", async () => {
    const { result } = renderHook(() => useTransactionList());
    await act(async () => {});
    await act(async () => result.current.toggleSortDirection());
    mockGetTransactions.mockResolvedValueOnce({
      status: "error",
      error: { code: "DatabaseError" },
    });
    await act(async () => result.current.toggleSortDirection());
    await act(async () => result.current.toggleSortDirection());
    expect(result.current.sortDirection).toBe("asc");

    await act(async () => result.current.retryTransactions());

    expect(mockGetJournal).toHaveBeenLastCalledWith(
      "account-1",
      expect.objectContaining({ newest_first: false }),
    );
  });

  // F29 — flipping the order keeps the rows on screen until the answer lands, and an
  // answer to an earlier order arriving late is dropped
  it("keeps the rows on screen while the order flips, and drops a late earlier answer", async () => {
    const { result } = renderHook(() => useTransactionList());
    await act(async () => {});
    let answerFirstFlip: (value: unknown) => void = () => {};
    mockGetTransactions
      .mockImplementationOnce(() => new Promise((resolve) => (answerFirstFlip = resolve)))
      .mockResolvedValueOnce({ status: "ok", data: [makeTx("tx-2", "2024-03-01")] });

    await act(async () => result.current.toggleSortDirection());
    expect(result.current.isLoadingTransactions).toBe(false);
    expect(result.current.sortedTransactions).toHaveLength(2);

    await act(async () => result.current.toggleSortDirection());
    await act(async () => {
      answerFirstFlip({ status: "ok", data: [] });
    });
    expect(result.current.sortedTransactions.map((r) => r.id)).toEqual(["tx-2"]);
  });

  // TXL-024 — flipping the order asks the core for the other order; the page does not reorder
  it("toggleSortDirection asks the core for the other order", async () => {
    const { result } = renderHook(() => useTransactionList());
    await act(async () => {});
    expect(result.current.sortDirection).toBe("desc");
    expect(mockGetJournal).toHaveBeenLastCalledWith(
      "account-1",
      expect.objectContaining({ asset_id: "asset-1", newest_first: true }),
    );

    await act(async () => result.current.toggleSortDirection());
    expect(result.current.sortDirection).toBe("asc");
    expect(mockGetJournal).toHaveBeenLastCalledWith(
      "account-1",
      expect.objectContaining({ newest_first: false }),
    );
    expect(result.current.sortedTransactions.at(0)?.date).toBe("2024-01-01");

    await act(async () => result.current.toggleSortDirection());
    expect(result.current.sortDirection).toBe("desc");
    expect(result.current.sortedTransactions.at(0)?.date).toBe("2024-03-01");
  });

  // handleAccountChange resets asset, sort, and re-fetches (TXL-012, TXL-016)
  it("handleAccountChange resets asset and fetches new asset IDs", async () => {
    const { result } = renderHook(() => useTransactionList());
    await act(async () => {});
    act(() => result.current.toggleSortDirection());
    expect(result.current.sortDirection).toBe("asc");

    await act(async () => {
      result.current.handleAccountChange("account-2");
    });

    expect(result.current.selectedAccountId).toBe("account-2");
    expect(result.current.selectedAssetId).toBeNull();
    expect(result.current.sortDirection).toBe("desc");
    expect(mockGetAssetIdsForAccount).toHaveBeenCalledWith("account-2");
  });

  // handleDeleteSuccess navigates back when list becomes empty (TXL-043)
  it("handleDeleteSuccess navigates to account when list is empty after delete", async () => {
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [] });
    const { result } = renderHook(() => useTransactionList());
    await act(async () => {});

    await act(async () => {
      await result.current.handleDeleteSuccess();
    });

    expect(mockNavigate).toHaveBeenCalledWith({
      to: "/accounts/$accountId",
      params: { accountId: "account-1" },
    });
  });

  // handleDeleteSuccess does not navigate when transactions remain (TXL-042)
  it("handleDeleteSuccess does not navigate when transactions remain", async () => {
    const { result } = renderHook(() => useTransactionList());
    await act(async () => {});

    await act(async () => {
      await result.current.handleDeleteSuccess();
    });

    expect(mockNavigate).not.toHaveBeenCalled();
  });

  // handleAssetChange resets sort direction (TXL-016)
  it("handleAssetChange resets sort direction to desc", async () => {
    const { result } = renderHook(() => useTransactionList());
    await act(async () => {});
    act(() => result.current.toggleSortDirection());
    expect(result.current.sortDirection).toBe("asc");

    await act(async () => {
      result.current.handleAssetChange("asset-2");
    });

    expect(result.current.selectedAssetId).toBe("asset-2");
    expect(result.current.sortDirection).toBe("desc");
    expect(mockGetTransactions).toHaveBeenCalledWith("account-1", "asset-2");
  });

  // transactionById provides O(1) lookup for raw transactions
  it("transactionById maps id to raw transaction", async () => {
    const { result } = renderHook(() => useTransactionList());
    await act(async () => {});
    expect(result.current.transactionById.get("tx-1")?.id).toBe("tx-1");
    expect(result.current.transactionById.get("tx-2")?.id).toBe("tx-2");
  });

  // A TransactionUpdated event re-fetches the list so a correction/move reflects
  // without navigating away and back.
  it("re-fetches transactions on a TransactionUpdated event", async () => {
    const { result } = renderHook(() => useTransactionList());
    await act(async () => {});
    expect(mockGetTransactions).toHaveBeenCalledTimes(1);

    mockGetTransactions.mockResolvedValue({
      status: "ok",
      data: [makeTx("tx-1", "2024-01-01")],
    });
    await act(async () => {
      eventCallback?.("TransactionUpdated");
    });

    expect(mockGetTransactions).toHaveBeenCalledTimes(2);
    expect(result.current.transactions).toHaveLength(1);
  });

  // TODO-020 / SYN-064 — changes applied from another device arrive as one SyncCompleted.
  it("re-fetches transactions once when a sync completes", async () => {
    renderHook(() => useTransactionList());
    await act(async () => {});
    expect(mockGetTransactions).toHaveBeenCalledTimes(1);

    await act(async () => {
      eventCallback?.("SyncCompleted");
    });

    expect(mockGetTransactions).toHaveBeenCalledTimes(2);
  });

  // Unrelated events do not trigger a re-fetch.
  it("ignores events other than TransactionUpdated", async () => {
    renderHook(() => useTransactionList());
    await act(async () => {});
    expect(mockGetTransactions).toHaveBeenCalledTimes(1);

    await act(async () => {
      eventCallback?.("AssetUpdated");
    });

    expect(mockGetTransactions).toHaveBeenCalledTimes(1);
  });
});
