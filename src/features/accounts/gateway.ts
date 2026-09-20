import {
  type Account,
  type AccountDeletionSummary,
  type AccountError,
  type AccountSummaries,
  type CreateAccountDTO,
  commands,
  type Event,
  events,
  type FetchAllAssetPricesError,
  type FetchTrigger,
  type Result,
  type UpdateAccountDTO,
} from "../../bindings";

/**
 * Gateway for Account-related backend communication.
 * Centralizes all Tauri command calls for the Account feature.
 */
export const accountGateway = {
  async getAccounts(): Promise<Result<Account[], AccountError>> {
    return await commands.getAccounts();
  },

  async getAccountSummaries(): Promise<Result<AccountSummaries, AccountError>> {
    return await commands.getAccountSummaries();
  },

  async addAccount(dto: CreateAccountDTO): Promise<Result<Account, AccountError>> {
    return await commands.addAccount(dto);
  },

  async updateAccount(dto: UpdateAccountDTO): Promise<Result<Account, AccountError>> {
    return await commands.updateAccount(dto);
  },

  async deleteAccount(id: string): Promise<Result<null, AccountError>> {
    return await commands.deleteAccount(id);
  },

  async getAccountDeletionSummary(
    accountId: string,
  ): Promise<Result<AccountDeletionSummary, AccountError>> {
    return await commands.getAccountDeletionSummary(accountId);
  },

  async fetchAllAssetPrices(
    trigger: FetchTrigger,
  ): Promise<Result<null, FetchAllAssetPricesError>> {
    return commands.fetchAllAssetPrices(trigger);
  },

  async subscribeToEvents(callback: (type: Event["type"]) => void): Promise<() => void> {
    return events.event.listen((event) => {
      callback(event.payload.type);
    });
  },

  // PMV-016 — the dialog needs the whole AssetPriceFetchCompleted payload, and
  // `subscribeToEvents` above strips every event down to its `type`. A second
  // listener carries the payload through intact; the caller decides what a null
  // `movement` means (PMV-010/014).
  async subscribeToPriceFetchCompleted(
    callback: (payload: Extract<Event, { type: "AssetPriceFetchCompleted" }>) => void,
  ): Promise<() => void> {
    return events.event.listen((event) => {
      if (event.payload.type === "AssetPriceFetchCompleted") {
        callback(event.payload);
      }
    });
  },
};
