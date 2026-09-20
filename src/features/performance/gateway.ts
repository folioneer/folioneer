import type {
  AccountDetailsResponse,
  AccountError,
  AccountPerformanceResponse,
  Event,
  Result,
} from "@/bindings";
import { commands, events } from "@/bindings";

export const accountPerformanceGateway = {
  async getAccountPerformance(
    accountId: string,
    assetId: string | null,
  ): Promise<Result<AccountPerformanceResponse, AccountError>> {
    return commands.getAccountPerformance(accountId, assetId);
  },

  /** PRF-082 — today's account holdings, the source of the asset-scope selector options. */
  async getAccountHoldings(
    accountId: string,
  ): Promise<Result<AccountDetailsResponse, AccountError>> {
    return commands.getAccountDetails(accountId, null);
  },

  async subscribeToEvents(callback: (type: Event["type"]) => void): Promise<() => void> {
    return events.event.listen((event) => {
      callback(event.payload.type);
    });
  },
};

export const globalPerformanceGateway = {
  /** GPF-010 — one command for every scope: all accounts, one account, one asset, or both. */
  async getGlobalPerformance(
    accountId: string | null,
    assetId: string | null,
  ): Promise<Result<AccountPerformanceResponse, AccountError>> {
    return commands.getGlobalPerformance(accountId, assetId);
  },

  /** Today's holdings of the scoped account, the source of its asset-scope selector options. */
  async getAccountHoldings(
    accountId: string,
  ): Promise<Result<AccountDetailsResponse, AccountError>> {
    return commands.getAccountDetails(accountId, null);
  },

  async subscribeToEvents(callback: (type: Event["type"]) => void): Promise<() => void> {
    return events.event.listen((event) => {
      callback(event.payload.type);
    });
  },
};
