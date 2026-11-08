import {
  type Account,
  type AccountDeletionSummary,
  type AccountError,
  type AccountSummaries,
  type CostBasisMethod,
  type CreateAccountDTO,
  commands,
  type Event,
  events,
  type FetchAllAssetPricesError,
  type FetchTrigger,
  type ImpliedCostBasis,
  type ProfileList,
  type Result,
  type UpdateAccountDTO,
  type WrapperLists,
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

  // ACC-040 — the wrapper lists; infallible, the lists are the application's own.
  async getWrapperLists(): Promise<WrapperLists> {
    return await commands.getWrapperLists();
  },

  // CBM-014 — the method a country implies, with the country it comes from; infallible.
  async getImpliedCostBasis(
    country: string | null,
    wrapper: string | null,
  ): Promise<ImpliedCostBasis> {
    return await commands.getImpliedCostBasis(country, wrapper);
  },

  // CBM-030 — puts an account on a method, its history recomputed.
  async setAccountCostBasisMethod(
    accountId: string,
    method: CostBasisMethod,
  ): Promise<Result<Account, AccountError>> {
    return await commands.setAccountCostBasisMethod(accountId, method);
  },

  // PFL-023 / PFL-012 — the profiles an account may belong to, and the profile in use.
  async listProfiles(): Promise<Result<ProfileList, AccountError>> {
    return await commands.listProfiles();
  },

  // PFL-031 — moves an account into another profile with its whole history.
  async moveAccountToProfile(
    accountId: string,
    profileId: string,
  ): Promise<Result<Account, AccountError>> {
    return await commands.moveAccountToProfile(accountId, profileId);
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
