import { useCallback } from "react";
import type { BuyHoldingDTO, SellHoldingDTO, Transaction } from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { transactionGateway } from "./gateway";
import { transactionMutationErrorToI18n } from "./shared/presenter";
import type { CorrectTransactionFields } from "./shared/types";

const UNKNOWN_ERROR: I18nMessage = { key: "error.Unknown" };

/**
 * Hook providing CRUD callbacks for the transaction feature.
 * Does not hold domain state — holdings are managed via the transaction store.
 */
export function useTransactions() {
  const buyHolding = useCallback(async (dto: BuyHoldingDTO) => {
    try {
      const res = await transactionGateway.buyHolding(dto);
      if (res.status === "ok") {
        return { data: res.data, error: null };
      }
      return { data: null, error: transactionMutationErrorToI18n(res.error) };
    } catch (e) {
      logger.error("Failed to buy holding", { error: e });
      return { data: null, error: UNKNOWN_ERROR };
    }
  }, []);

  const sellHolding = useCallback(async (dto: SellHoldingDTO) => {
    try {
      const res = await transactionGateway.sellHolding(dto);
      if (res.status === "ok") {
        return { data: res.data, error: null };
      }
      return { data: null, error: transactionMutationErrorToI18n(res.error) };
    } catch (e) {
      logger.error("Failed to sell holding", { error: e });
      return { data: null, error: UNKNOWN_ERROR };
    }
  }, []);

  const correctTransaction = useCallback(
    async (id: string, accountId: string, dto: CorrectTransactionFields) => {
      try {
        const res = await transactionGateway.correctTransaction(id, accountId, dto);
        if (res.status === "ok") {
          return { data: res.data, error: null };
        }
        return { data: null, error: transactionMutationErrorToI18n(res.error) };
      } catch (e) {
        logger.error("Failed to correct transaction", { error: e });
        return { data: null, error: UNKNOWN_ERROR };
      }
    },
    [],
  );

  const cancelTransaction = useCallback(async (id: string, accountId: string) => {
    try {
      const res = await transactionGateway.cancelTransaction(id, accountId);
      if (res.status === "ok") {
        return { error: null };
      }
      return { error: transactionMutationErrorToI18n(res.error) };
    } catch (e) {
      logger.error("Failed to cancel transaction", { error: e });
      return { error: UNKNOWN_ERROR };
    }
  }, []);

  const getTransactions = useCallback(
    async (accountId: string, assetId: string): Promise<Transaction[]> => {
      try {
        const res = await transactionGateway.getTransactions(accountId, assetId);
        if (res.status === "ok") {
          return res.data;
        }
        logger.error("Failed to get transactions", { error: res.error });
        return [];
      } catch (e) {
        logger.error("Failed to get transactions", { error: e });
        return [];
      }
    },
    [],
  );

  return {
    buyHolding,
    sellHolding,
    correctTransaction,
    cancelTransaction,
    getTransactions,
  };
}
