import type { KeyboardEvent, MouseEvent } from "react";
import { useCallback, useMemo, useState } from "react";
import type { Account, AccountDeletionSummary, AccountSummary } from "@/bindings";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";
import { FREQUENCY_ORDER } from "../shared/presenter";

const UNKNOWN_ERROR: I18nMessage = { key: "error.Unknown" };

export type SortConfig = {
  key:
    | "name"
    | "bank_name"
    | "update_frequency"
    | "total_global_value"
    | "total_unrealized_pnl"
    | "ytd_performance_pct";
  direction: "asc" | "desc";
};

export function useAccountTable(
  accounts: AccountSummary[],
  searchTerm: string,
  deleteAccount: (id: string) => Promise<{ error: I18nMessage | null }>,
  getAccountDeletionSummary: (
    id: string,
  ) => Promise<{ data: AccountDeletionSummary | null; error: I18nMessage | null }>,
  onAccountClick: (accountId: string) => void,
) {
  const [sortConfig, setSortConfig] = useState<SortConfig>({
    key: "name",
    direction: "asc",
  });
  const [deleteData, setDeleteData] = useState<{
    id: string;
    name: string;
  } | null>(null);
  const [deleteSummary, setDeleteSummary] = useState<AccountDeletionSummary | null>(null);
  const [fetchingSummaryFor, setFetchingSummaryFor] = useState<string | null>(null);
  const [editData, setEditData] = useState<Account | null>(null);
  const [actionError, setActionError] = useState<I18nMessage | null>(null);

  const handleSort = useCallback((key: SortConfig["key"]) => {
    setSortConfig((prev) => ({
      key,
      direction: prev.key === key && prev.direction === "asc" ? "desc" : "asc",
    }));
  }, []);

  const handleNameKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        handleSort("name");
      }
    },
    [handleSort],
  );

  const handleBankNameKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        handleSort("bank_name");
      }
    },
    [handleSort],
  );

  const handleFrequencyKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        handleSort("update_frequency");
      }
    },
    [handleSort],
  );

  const handleGlobalValueKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        handleSort("total_global_value");
      }
    },
    [handleSort],
  );

  const handleUnrealizedPnlKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        handleSort("total_unrealized_pnl");
      }
    },
    [handleSort],
  );

  const handleYtdPctKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        handleSort("ytd_performance_pct");
      }
    },
    [handleSort],
  );

  const handleRowKeyDown = useCallback(
    (e: KeyboardEvent, accountId: string) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        onAccountClick(accountId);
      }
    },
    [onAccountClick],
  );

  const storeAccounts = useAppStore((state) => state.accounts);

  // ACC-026 — the summaries DTO does not carry bank_name; it is resolved from
  // the loaded account catalog by account id.
  const bankNameByAccountId = useMemo(() => {
    const lookup = new Map<string, string>();
    for (const catalogAccount of storeAccounts) {
      lookup.set(catalogAccount.id, catalogAccount.bank_name);
    }
    return lookup;
  }, [storeAccounts]);

  const handleEditClick = useCallback(
    (e: MouseEvent, account: AccountSummary) => {
      e.stopPropagation();
      // EditAccountModal expects a full Account; the summary row lacks
      // bank_name (ACC-026) and management_fees_enabled (FEE-075), so both are
      // read from the loaded account catalog to prefill the edit form correctly.
      const { id, name, currency, update_frequency } = account;
      const catalogAccount = storeAccounts.find((a) => a.id === id);
      const bank_name = catalogAccount?.bank_name ?? "";
      const management_fees_enabled = catalogAccount?.management_fees_enabled ?? false;
      setEditData({ id, name, bank_name, currency, update_frequency, management_fees_enabled });
    },
    [storeAccounts],
  );

  const handleEditClose = useCallback(() => setEditData(null), []);

  const handleDeleteClick = useCallback(
    async (e: MouseEvent, id: string, name: string) => {
      e.stopPropagation();
      setFetchingSummaryFor(id);
      setActionError(null);
      const result = await getAccountDeletionSummary(id);
      setFetchingSummaryFor(null);
      if (result.error) {
        setActionError(result.error);
        return;
      }
      if (!result.data) {
        setActionError(UNKNOWN_ERROR);
        return;
      }
      setDeleteSummary(result.data);
      setDeleteData({ id, name });
    },
    [getAccountDeletionSummary],
  );

  const handleDeleteCancel = useCallback(() => {
    setDeleteData(null);
    setDeleteSummary(null);
  }, []);

  const handleDeleteConfirm = useCallback(async () => {
    if (!deleteData) return;
    const result = await deleteAccount(deleteData.id);
    if (result.error) {
      // R13 — keep dialog open, show inline error
      setActionError(result.error);
    } else {
      setDeleteData(null);
      setDeleteSummary(null);
    }
  }, [deleteData, deleteAccount]);

  const sortedAndFilteredAccounts = useMemo(() => {
    const filtered = accounts
      .filter((a) => a.name.toLowerCase().includes(searchTerm.toLowerCase()))
      .map((a) => ({ ...a, bank_name: bankNameByAccountId.get(a.id) ?? "" }));

    const byName = (a: AccountSummary, b: AccountSummary) =>
      a.name.toLowerCase().localeCompare(b.name.toLowerCase());

    return [...filtered].sort((a, b) => {
      // ACC-026 + ACC-008 — bank name with empty-string-as-null semantics:
      // empty values always sort last, independent of direction.
      if (sortConfig.key === "bank_name") {
        const av = a.bank_name;
        const bv = b.bank_name;
        if (av === "" && bv === "") return byName(a, b);
        if (av === "") return 1;
        if (bv === "") return -1;
        const cmp = av.toLowerCase().localeCompare(bv.toLowerCase());
        if (cmp === 0) return byName(a, b);
        return sortConfig.direction === "asc" ? cmp : -cmp;
      }

      // ACC-008 — nullable metric columns: null values always sort last,
      // independent of direction (so they stay at the bottom in both asc & desc).
      if (sortConfig.key === "total_unrealized_pnl" || sortConfig.key === "ytd_performance_pct") {
        const av = a[sortConfig.key];
        const bv = b[sortConfig.key];
        if (av === null && bv === null) return byName(a, b);
        if (av === null) return 1;
        if (bv === null) return -1;
        const cmp = av - bv;
        if (cmp === 0) return byName(a, b);
        return sortConfig.direction === "asc" ? cmp : -cmp;
      }

      let cmp: number;
      if (sortConfig.key === "update_frequency") {
        // R9 — sort by logical enum order, not alphabetical label
        cmp = FREQUENCY_ORDER[a.update_frequency] - FREQUENCY_ORDER[b.update_frequency];
      } else if (sortConfig.key === "total_global_value") {
        // ACC-008 — numeric compare on micros; ties broken by name asc for stability
        cmp = a.total_global_value - b.total_global_value;
        if (cmp === 0) cmp = byName(a, b);
      } else {
        cmp = byName(a, b);
      }
      return sortConfig.direction === "asc" ? cmp : -cmp;
    });
  }, [accounts, searchTerm, sortConfig, bankNameByAccountId]);

  // R11 — no accounts exist and no search is active
  const isEmpty = accounts.length === 0 && searchTerm.trim().length === 0;

  // R10 — search is active but no results match
  const hasNoSearchResults = searchTerm.trim().length > 0 && sortedAndFilteredAccounts.length === 0;

  return {
    sortedAndFilteredAccounts,
    sortConfig,
    handleSort,
    handleNameKeyDown,
    handleBankNameKeyDown,
    handleFrequencyKeyDown,
    handleGlobalValueKeyDown,
    handleUnrealizedPnlKeyDown,
    handleYtdPctKeyDown,
    handleRowKeyDown,
    handleEditClick,
    handleEditClose,
    handleDeleteClick,
    handleDeleteCancel,
    isEmpty,
    hasNoSearchResults,
    deleteData,
    deleteSummary,
    fetchingSummaryFor,
    editData,
    actionError,
    setActionError,
    handleDeleteConfirm,
  };
}
