import { useParams } from "@tanstack/react-router";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { Transaction } from "@/bindings";
import { accountMutationErrorToI18n } from "@/features/accounts/shared/presenter";
import { logger } from "@/lib/logger";
import { decimalToMicro } from "@/lib/microUnits";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";
import { transactionGateway } from "../gateway";
import {
  type TransactionRowViewModel,
  toCashStatementCells,
  toTransactionRow,
} from "../shared/presenter";

const UNKNOWN_ERROR: I18nMessage = { key: "error.Unknown" };

/** All filters are AND-combined; empty values mean "no constraint". */
interface JournalFilters {
  assetId: string;
  type: string;
  amountMin: string;
  amountMax: string;
}

const EMPTY_FILTERS: JournalFilters = { assetId: "", type: "", amountMin: "", amountMax: "" };

export function useAccountJournal() {
  const { accountId } = useParams({ from: "/accounts/$accountId/journal" });
  const assets = useAppStore((s) => s.assets);
  const accounts = useAppStore((s) => s.accounts);

  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<I18nMessage | null>(null);
  // Chronological, latest first (the journal default).
  const [sortDirection, setSortDirection] = useState<"asc" | "desc">("desc");
  const [filters, setFilters] = useState<JournalFilters>(EMPTY_FILTERS);

  const fetchTransactions = useCallback(async (): Promise<void> => {
    setIsLoading(true);
    setError(null);
    try {
      const res = await transactionGateway.getAllTransactionsForAccount(accountId);
      if (res.status === "ok") {
        setTransactions(res.data);
      } else {
        setError(accountMutationErrorToI18n(res.error));
        setTransactions([]);
      }
    } catch (e) {
      logger.error("Failed to fetch account journal", { error: e });
      setError(UNKNOWN_ERROR);
      setTransactions([]);
    } finally {
      setIsLoading(false);
    }
  }, [accountId]);

  useEffect(() => {
    fetchTransactions();
  }, [fetchTransactions]);

  // Re-fetch on TransactionUpdated so an edit/delete reflects without navigating away,
  // and once on SyncCompleted for everything a sync applied (SYN-064).
  useEffect(() => {
    const unlistenPromise = transactionGateway.subscribeToEvents((type) => {
      if (type === "TransactionUpdated" || type === "SyncCompleted") {
        fetchTransactions();
      }
    });
    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [fetchTransactions]);

  const setFilter = useCallback((field: keyof JournalFilters, value: string) => {
    setFilters((prev) => ({ ...prev, [field]: value }));
  }, []);

  const clearFilters = useCallback(() => setFilters(EMPTY_FILTERS), []);

  const toggleSortDirection = useCallback(() => {
    setSortDirection((d) => (d === "asc" ? "desc" : "asc"));
  }, []);

  const transactionById = useMemo(() => {
    const map = new Map<string, Transaction>();
    for (const tx of transactions) map.set(tx.id, tx);
    return map;
  }, [transactions]);

  // Asset options come from the transactions actually present (TXL-013 style).
  const assetFilterOptions = useMemo(() => {
    const ids = [...new Set(transactions.map((tx) => tx.asset_id))];
    return ids.map((id) => ({ value: id, label: assets.find((a) => a.id === id)?.name ?? id }));
  }, [transactions, assets]);

  const typeFilterOptions = useMemo(() => {
    return [...new Set(transactions.map((tx) => tx.transaction_type))].map((type) => ({
      value: type,
      label: type,
    }));
  }, [transactions]);

  // Bank-statement cash columns. Computed over the FULL chronological set (date ASC,
  // created_at ASC) so each row's balance is the true cash balance at that point —
  // independent of the display filters/sort. Signs mirror the backend cash replay:
  // credit = Deposit/Sell/Dividend, debit = Withdrawal/Purchase, none = OpeningBalance/FreeShares.
  const cashByTxId = useMemo(() => {
    const ordered = [...transactions].sort((a, b) =>
      a.date !== b.date ? a.date.localeCompare(b.date) : a.created_at.localeCompare(b.created_at),
    );
    const map = new Map<string, { cashOut: string; cashIn: string; balance: string }>();
    let running = 0;
    for (const tx of ordered) {
      const isCredit =
        tx.transaction_type === "Deposit" ||
        tx.transaction_type === "Sell" ||
        tx.transaction_type === "Dividend";
      const isDebit = tx.transaction_type === "Withdrawal" || tx.transaction_type === "Purchase";
      if (isCredit) running += tx.total_amount;
      else if (isDebit) running -= tx.total_amount;
      // Raw micro values → presenter formats them (F5); the arithmetic stays here.
      map.set(
        tx.id,
        toCashStatementCells({
          debitMicros: isDebit ? tx.total_amount : null,
          creditMicros: isCredit ? tx.total_amount : null,
          balanceMicros: running,
        }),
      );
    }
    return map;
  }, [transactions]);

  const filteredSortedRows = useMemo<TransactionRowViewModel[]>(() => {
    const hasMin = filters.amountMin.trim() !== "";
    const hasMax = filters.amountMax.trim() !== "";
    const minMicro = hasMin ? decimalToMicro(filters.amountMin) : 0;
    const maxMicro = hasMax ? decimalToMicro(filters.amountMax) : 0;

    const filtered = transactions.filter((tx) => {
      if (filters.assetId && tx.asset_id !== filters.assetId) return false;
      if (filters.type && tx.transaction_type !== filters.type) return false;
      if (hasMin && tx.total_amount < minMicro) return false;
      if (hasMax && tx.total_amount > maxMicro) return false;
      return true;
    });

    // Order on the raw transactions (which carry created_at) so same-date events
    // tie-break by input order — matching the running-balance replay. Sorting rows
    // by date alone would leave same-date rows in their stable input order even in
    // desc view, flipping the balance column at each day boundary.
    const ordered = [...filtered].sort((a, b) => {
      const cmp = a.date.localeCompare(b.date) || a.created_at.localeCompare(b.created_at);
      return sortDirection === "asc" ? cmp : -cmp;
    });

    return ordered.map((tx) => {
      const asset = assets.find((a) => a.id === tx.asset_id);
      const account = accounts.find((a) => a.id === tx.account_id);
      const row = toTransactionRow(tx, asset?.name ?? tx.asset_id, account?.name ?? tx.account_id);
      const cash = cashByTxId.get(tx.id);
      return cash ? { ...row, ...cash } : row;
    });
  }, [transactions, filters, assets, accounts, sortDirection, cashByTxId]);

  return {
    accountId,
    isLoading,
    error,
    sortDirection,
    filters,
    setFilter,
    clearFilters,
    toggleSortDirection,
    assetFilterOptions,
    typeFilterOptions,
    filteredSortedRows,
    transactionById,
    hasTransactions: transactions.length > 0,
    refresh: fetchTransactions,
  };
}
