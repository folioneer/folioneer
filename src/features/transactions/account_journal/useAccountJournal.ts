import { useParams } from "@tanstack/react-router";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AccountJournal, JournalFilter, Transaction, TransactionType } from "@/bindings";
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

/** The form's filters and order as the account journal query takes them (TXL-060). */
function toJournalFilter(filters: JournalFilters, sortDirection: "asc" | "desc"): JournalFilter {
  return {
    asset_id: filters.assetId || null,
    transaction_type: (filters.type || null) as TransactionType | null,
    amount_min: filters.amountMin.trim() !== "" ? decimalToMicro(filters.amountMin) : null,
    amount_max: filters.amountMax.trim() !== "" ? decimalToMicro(filters.amountMax) : null,
    newest_first: sortDirection === "desc",
  };
}

/**
 * TXL-061 — the account journal page: the core orders, filters and computes the cash
 * columns (TXL-060); this hook sends the filters and shows what comes back.
 */
export function useAccountJournal() {
  const { accountId } = useParams({ from: "/accounts/$accountId/journal" });
  const assets = useAppStore((s) => s.assets);
  const accounts = useAppStore((s) => s.accounts);

  const [journal, setJournal] = useState<AccountJournal | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<I18nMessage | null>(null);
  // Chronological, latest first (the journal default).
  const [sortDirection, setSortDirection] = useState<"asc" | "desc">("desc");
  const [filters, setFilters] = useState<JournalFilters>(EMPTY_FILTERS);
  const filter = useMemo(() => toJournalFilter(filters, sortDirection), [filters, sortDirection]);
  // Only the answer to the latest request is shown; an earlier one arriving late is dropped.
  const latestRequest = useRef(0);

  const fetchJournal = useCallback(async (): Promise<void> => {
    const request = ++latestRequest.current;
    setError(null);
    try {
      const res = await transactionGateway.getAccountJournal(accountId, filter);
      if (request !== latestRequest.current) return;
      if (res.status === "ok") {
        setJournal(res.data);
      } else {
        setError(accountMutationErrorToI18n(res.error));
        setJournal(null);
      }
    } catch (e) {
      if (request !== latestRequest.current) return;
      logger.error("Failed to fetch account journal", { error: e });
      setError(UNKNOWN_ERROR);
      setJournal(null);
    } finally {
      if (request === latestRequest.current) setIsLoading(false);
    }
  }, [accountId, filter]);

  // F29 — the loading state shows for the first load and a change of account only; a
  // change of filter or order keeps the current rows on screen until the answer lands.
  const shownAccount = useRef<string | null>(null);
  useEffect(() => {
    if (shownAccount.current !== accountId) {
      shownAccount.current = accountId;
      setIsLoading(true);
    }
    fetchJournal();
  }, [accountId, fetchJournal]);

  // Re-fetch on TransactionUpdated so an edit/delete reflects without navigating away,
  // and once on SyncCompleted for everything a sync applied (SYN-064).
  useEffect(() => {
    const unlistenPromise = transactionGateway.subscribeToEvents((type) => {
      if (type === "TransactionUpdated" || type === "SyncCompleted") {
        fetchJournal();
      }
    });
    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [fetchJournal]);

  const setFilter = useCallback((field: keyof JournalFilters, value: string) => {
    setFilters((prev) => ({ ...prev, [field]: value }));
  }, []);

  const clearFilters = useCallback(() => setFilters(EMPTY_FILTERS), []);

  const toggleSortDirection = useCallback(() => {
    setSortDirection((d) => (d === "asc" ? "desc" : "asc"));
  }, []);

  const transactionById = useMemo(() => {
    const map = new Map<string, Transaction>();
    for (const row of journal?.rows ?? []) map.set(row.transaction.id, row.transaction);
    return map;
  }, [journal]);

  // The filter choices are the assets and types the whole account has (TXL-060).
  const assetFilterOptions = useMemo(
    () =>
      (journal?.asset_ids ?? []).map((id) => ({
        value: id,
        label: assets.find((a) => a.id === id)?.name ?? id,
      })),
    [journal, assets],
  );

  const typeFilterOptions = useMemo(
    () => (journal?.transaction_types ?? []).map((type) => ({ value: type, label: type })),
    [journal],
  );

  const filteredSortedRows = useMemo<TransactionRowViewModel[]>(
    () =>
      (journal?.rows ?? []).map((row) => {
        const tx = row.transaction;
        const asset = assets.find((a) => a.id === tx.asset_id);
        const account = accounts.find((a) => a.id === tx.account_id);
        return {
          ...toTransactionRow(tx, asset?.name ?? tx.asset_id, account?.name ?? tx.account_id),
          ...toCashStatementCells({
            debitMicros: row.cash_out,
            creditMicros: row.cash_in,
            balanceMicros: row.cash_balance,
          }),
        };
      }),
    [journal, assets, accounts],
  );

  // F29 — a retry the user asks for shows the loading state; a re-fetch after a change does not.
  const reloadTransactions = useCallback(() => {
    setIsLoading(true);
    return fetchJournal();
  }, [fetchJournal]);

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
    hasTransactions: journal?.has_transactions ?? false,
    refresh: fetchJournal,
    reload: reloadTransactions,
  };
}
