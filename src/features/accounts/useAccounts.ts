import { useCallback } from "react";
import type { CreateAccountDTO, UpdateAccountDTO } from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { useAppStore } from "../../lib/store";
import { accountGateway } from "./gateway";
import { accountMutationErrorToI18n } from "./shared/presenter";

const UNKNOWN_ERROR: I18nMessage = { key: "error.Unknown" };

export function useAccounts() {
  const accounts = useAppStore((state) => state.accounts);
  const loading = useAppStore((state) => state.isLoadingAccounts);
  const fetchError = useAppStore((state) => state.accountsError);
  const fetchAccounts = useAppStore((state) => state.fetchAccounts);

  const addAccount = useCallback(async (dto: CreateAccountDTO) => {
    try {
      const res = await accountGateway.addAccount(dto);
      if (res.status === "ok") {
        return { data: res.data, error: null };
      }
      return { data: null, error: accountMutationErrorToI18n(res.error) };
    } catch (e) {
      logger.error("Failed to add account", { error: e });
      return { data: null, error: UNKNOWN_ERROR };
    }
  }, []);

  const updateAccount = useCallback(async (dto: UpdateAccountDTO) => {
    try {
      const res = await accountGateway.updateAccount(dto);
      if (res.status === "ok") {
        return { data: res.data, error: null };
      }
      return { data: null, error: accountMutationErrorToI18n(res.error) };
    } catch (e) {
      logger.error("Failed to update account", { error: e });
      return { data: null, error: UNKNOWN_ERROR };
    }
  }, []);

  const deleteAccount = useCallback(async (id: string) => {
    try {
      const res = await accountGateway.deleteAccount(id);
      if (res.status === "ok") {
        return { error: null };
      }
      return { error: accountMutationErrorToI18n(res.error) };
    } catch (e) {
      logger.error("Failed to delete account", { error: e });
      return { error: UNKNOWN_ERROR };
    }
  }, []);

  const getAccountDeletionSummary = useCallback(async (id: string) => {
    try {
      const res = await accountGateway.getAccountDeletionSummary(id);
      if (res.status === "ok") {
        return { data: res.data, error: null };
      }
      return { data: null, error: accountMutationErrorToI18n(res.error) };
    } catch (e) {
      logger.error("Failed to fetch account deletion summary", { error: e });
      return { data: null, error: UNKNOWN_ERROR };
    }
  }, []);

  return {
    accounts,
    loading,
    fetchError,
    fetchAccounts,
    addAccount,
    updateAccount,
    deleteAccount,
    getAccountDeletionSummary,
  };
}
