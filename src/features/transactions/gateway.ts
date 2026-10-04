import type {
  AccountError,
  AccountJournal,
  Asset,
  AssetError,
  BuyHoldingDTO,
  Event,
  JournalFilter,
  OpeningBalanceDraft,
  OpeningBalancePreview,
  SellHoldingDTO,
  Transaction,
  TransactionDraft,
  TransactionDraftError,
  TransactionPreview,
} from "../../bindings";
import { commands, events, type Result } from "../../bindings";
import type { CorrectTransactionFields } from "./shared/types";

/**
 * Gateway for Transaction-related backend communication.
 * Centralizes all Tauri command calls for the Transaction feature.
 */
export const transactionGateway = {
  async buyHolding(dto: BuyHoldingDTO): Promise<Result<Transaction, AccountError>> {
    return await commands.buyHolding(dto);
  },

  async sellHolding(dto: SellHoldingDTO): Promise<Result<Transaction, AccountError>> {
    return await commands.sellHolding(dto);
  },

  async correctTransaction(
    id: string,
    accountId: string,
    dto: CorrectTransactionFields,
  ): Promise<Result<Transaction, AccountError>> {
    return await commands.correctTransaction({ ...dto, account_id: accountId, transaction_id: id });
  },

  async cancelTransaction(id: string, accountId: string): Promise<Result<null, AccountError>> {
    return await commands.cancelTransaction({ account_id: accountId, transaction_id: id });
  },

  async validateTransactionDraft(
    draft: TransactionDraft,
  ): Promise<Result<TransactionPreview, TransactionDraftError>> {
    return await commands.validateTransactionDraft(draft);
  },

  // TRX-066 — checks an opening balance draft, new or corrected, without writing.
  async validateOpeningBalanceDraft(
    draft: OpeningBalanceDraft,
  ): Promise<Result<OpeningBalancePreview, TransactionDraftError>> {
    return await commands.validateOpeningBalanceDraft(draft);
  },

  async getTransactions(
    accountId: string,
    assetId: string,
  ): Promise<Result<Transaction[], AccountError>> {
    return await commands.getTransactions(accountId, assetId);
  },

  async getAccountJournal(
    accountId: string,
    filter: JournalFilter,
  ): Promise<Result<AccountJournal, AccountError>> {
    return await commands.getAccountJournal(accountId, filter);
  },

  async getNonCashAssets(): Promise<Result<Asset[], AssetError>> {
    return await commands.getNonCashAssets();
  },

  async getAssetIdsForAccount(accountId: string): Promise<Result<string[], AccountError>> {
    return await commands.getAssetIdsForAccount(accountId);
  },

  async recordAssetPrice(
    assetId: string,
    date: string,
    price: number,
  ): Promise<Result<null, AssetError>> {
    return await commands.recordAssetPrice(assetId, date, price);
  },

  /** Subscribe to the backend event bus; invokes `callback` with each event's discriminant. */
  async subscribeToEvents(callback: (type: Event["type"]) => void): Promise<() => void> {
    return events.event.listen((event) => {
      callback(event.payload.type);
    });
  },
};
