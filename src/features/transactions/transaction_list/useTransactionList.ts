import { useNavigate, useParams } from "@tanstack/react-router";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Transaction } from "@/bindings";
import { accountMutationErrorToI18n } from "@/features/accounts/shared/presenter";
import { logger } from "@/lib/logger";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";
import { transactionGateway } from "../gateway";
import { type TransactionRowViewModel, toTransactionRow } from "../shared/presenter";

const UNKNOWN_ERROR: I18nMessage = { key: "error.Unknown" };

export function useTransactionList() {
  const { accountId, assetId } = useParams({
    from: "/accounts/$accountId/transactions/$assetId",
  });
  const navigate = useNavigate();

  const accounts = useAppStore((s) => s.accounts);
  const assets = useAppStore((s) => s.assets);

  const [selectedAccountId, setSelectedAccountId] = useState(accountId);
  const [selectedAssetId, setSelectedAssetId] = useState<string | null>(assetId);

  const [assetIdsForAccount, setAssetIdsForAccount] = useState<string[]>([]);
  const [isLoadingAssets, setIsLoadingAssets] = useState(false);
  const [assetListError, setAssetListError] = useState<I18nMessage | null>(null);

  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [isLoadingTransactions, setIsLoadingTransactions] = useState(false);
  const [transactionError, setTransactionError] = useState<I18nMessage | null>(null);

  const [sortDirection, setSortDirection] = useState<"asc" | "desc">("desc");

  const fetchAssetIds = useCallback(async (accId: string) => {
    setIsLoadingAssets(true);
    setAssetListError(null);
    try {
      const res = await transactionGateway.getAssetIdsForAccount(accId);
      if (res.status === "ok") {
        setAssetIdsForAccount(res.data);
      } else {
        setAssetListError(accountMutationErrorToI18n(res.error));
        setAssetIdsForAccount([]);
      }
    } catch (e) {
      logger.error("Failed to fetch asset IDs", { error: e });
      setAssetListError(UNKNOWN_ERROR);
      setAssetIdsForAccount([]);
    } finally {
      setIsLoadingAssets(false);
    }
  }, []);

  // TXL-024 / TXL-060 — the asset's transactions come from the account journal, in the
  // order asked for; the page does not reorder them.
  // Only the answer to the latest request is shown; an earlier one arriving late is dropped.
  const latestRequest = useRef(0);
  // F29 — the loading state shows for a first load or a change of asset only; a flip of the
  // order or a refresh after a change keeps the current rows on screen.
  const fetchTransactions = useCallback(
    async (
      accId: string,
      asId: string,
      newestFirst = true,
      showLoading = true,
    ): Promise<Transaction[]> => {
      const request = ++latestRequest.current;
      if (showLoading) setIsLoadingTransactions(true);
      setTransactionError(null);
      try {
        const res = await transactionGateway.getAccountJournal(accId, {
          asset_id: asId,
          transaction_type: null,
          amount_min: null,
          amount_max: null,
          newest_first: newestFirst,
        });
        const found = res.status === "ok" ? res.data.rows.map((row) => row.transaction) : [];
        if (request !== latestRequest.current) return found;
        if (res.status === "ok") {
          setTransactions(found);
          return found;
        }
        setTransactionError(accountMutationErrorToI18n(res.error));
        setTransactions([]);
        return [];
      } catch (e) {
        if (request !== latestRequest.current) return [];
        logger.error("Failed to fetch transactions", { error: e });
        setTransactionError(UNKNOWN_ERROR);
        setTransactions([]);
        return [];
      } finally {
        if (request === latestRequest.current) setIsLoadingTransactions(false);
      }
    },
    [],
  );

  // Fetch on mount and whenever route params change (TXL-011)
  useEffect(() => {
    fetchAssetIds(accountId);
    fetchTransactions(accountId, assetId);
  }, [accountId, assetId, fetchAssetIds, fetchTransactions]);

  const handleAccountChange = useCallback(
    (newAccountId: string) => {
      setSelectedAccountId(newAccountId);
      setSelectedAssetId(null);
      setTransactions([]);
      setTransactionError(null);
      setSortDirection("desc");
      fetchAssetIds(newAccountId);
    },
    [fetchAssetIds],
  );

  const handleAssetChange = useCallback(
    (newAssetId: string) => {
      setSelectedAssetId(newAssetId);
      setSortDirection("desc");
      fetchTransactions(selectedAccountId, newAssetId);
    },
    [selectedAccountId, fetchTransactions],
  );

  const toggleSortDirection = useCallback(() => {
    const next = sortDirection === "asc" ? "desc" : "asc";
    setSortDirection(next);
    if (selectedAssetId) {
      fetchTransactions(selectedAccountId, selectedAssetId, next === "desc", false);
    }
  }, [sortDirection, selectedAccountId, selectedAssetId, fetchTransactions]);

  const refreshTransactions = useCallback(
    async (preserveSort = true): Promise<Transaction[]> => {
      if (!selectedAssetId) return [];
      if (!preserveSort) setSortDirection("desc");
      const newestFirst = !preserveSort || sortDirection === "desc";
      return fetchTransactions(selectedAccountId, selectedAssetId, newestFirst, false);
    },
    [selectedAccountId, selectedAssetId, sortDirection, fetchTransactions],
  );

  // Re-fetch on TransactionUpdated so a correction/move reflects without a navigate-away,
  // and once on SyncCompleted for everything a sync applied (SYN-064).
  useEffect(() => {
    const unlistenPromise = transactionGateway.subscribeToEvents((type) => {
      if (type === "TransactionUpdated" || type === "SyncCompleted") {
        refreshTransactions();
      }
    });
    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refreshTransactions]);

  const handleDeleteSuccess = useCallback(async () => {
    const remaining = await refreshTransactions();
    if (remaining.length === 0) {
      navigate({
        to: "/accounts/$accountId",
        params: { accountId: selectedAccountId },
      });
    }
  }, [refreshTransactions, selectedAccountId, navigate]);

  const handleEditSuccess = useCallback(async () => {
    await refreshTransactions();
  }, [refreshTransactions]);

  const retryAssetList = useCallback(() => {
    fetchAssetIds(selectedAccountId);
  }, [selectedAccountId, fetchAssetIds]);

  // A retry keeps the order the user chose (TXL-024) and shows the loading state (F29).
  const retryTransactions = useCallback(() => {
    if (selectedAssetId) {
      fetchTransactions(selectedAccountId, selectedAssetId, sortDirection === "desc");
    }
  }, [selectedAccountId, selectedAssetId, sortDirection, fetchTransactions]);

  const rows = useMemo<TransactionRowViewModel[]>(() => {
    return transactions.map((tx) => {
      const asset = assets.find((a) => a.id === tx.asset_id);
      const account = accounts.find((a) => a.id === tx.account_id);
      return toTransactionRow(tx, asset?.name ?? tx.asset_id, account?.name ?? tx.account_id);
    });
  }, [transactions, assets, accounts]);

  const sortedTransactions = rows;

  const transactionById = useMemo(() => {
    const map = new Map<string, Transaction>();
    for (const tx of transactions) map.set(tx.id, tx);
    return map;
  }, [transactions]);

  const assetOptions = useMemo(() => {
    return assetIdsForAccount.map((id) => {
      const asset = assets.find((a) => a.id === id);
      return { value: id, label: asset?.name ?? id };
    });
  }, [assetIdsForAccount, assets]);

  const accountOptions = useMemo(() => {
    return accounts.map((a) => ({ value: a.id, label: a.name }));
  }, [accounts]);

  return {
    selectedAccountId,
    selectedAssetId,
    accountOptions,
    assetOptions,
    isLoadingAssets,
    assetListError,
    isLoadingTransactions,
    transactionError,
    sortDirection,
    sortedTransactions,
    transactions,
    transactionById,
    handleAccountChange,
    handleAssetChange,
    toggleSortDirection,
    refreshTransactions,
    handleDeleteSuccess,
    handleEditSuccess,
    retryAssetList,
    retryTransactions,
  };
}
