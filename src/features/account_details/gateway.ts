import type {
  AccountDetailsResponse,
  AccountError,
  AssetError,
  AssetPrice,
  CreateFeeScheduleDTO,
  DeleteHoldingNoteDTO,
  DepositDTO,
  DividendDTO,
  DividendError,
  Event,
  FeeSchedule,
  FetchAccountAssetPricesError,
  FreeSharesDTO,
  FreeSharesError,
  HoldingNote,
  HoldingSnapshot,
  InterestError,
  ManagementFeeDTO,
  ManagementFeeError,
  ManagementFeeRemoval,
  OpenHoldingDTO,
  OpenHoldingError,
  PriceHistoryBackfillError,
  PriceHistoryBackfillOutcome,
  RecordInterestDTO,
  RecordSplitDTO,
  Result,
  SplitError,
  Transaction,
  UpdateFeeScheduleDTO,
  UpsertHoldingNoteDTO,
  WithdrawalDTO,
} from "@/bindings";
import { commands, events } from "@/bindings";
import type { CorrectTransactionFields } from "@/features/transactions/shared/types";
import { useAppStore } from "@/lib/store";

export const accountDetailsGateway = {
  // `asOfDate` selects the valuation date: null/omitted is the live view (today),
  // an ISO "YYYY-MM-DD" reconstructs the account read-only as of that past date.
  async getAccountDetails(
    accountId: string,
    asOfDate?: string | null,
  ): Promise<Result<AccountDetailsResponse, AccountError>> {
    return commands.getAccountDetails(accountId, asOfDate ?? null);
  },

  async recordAssetPrice(
    assetId: string,
    date: string,
    price: number,
  ): Promise<Result<null, AssetError>> {
    return commands.recordAssetPrice(assetId, date, price);
  },

  async getAssetPrices(assetId: string): Promise<Result<AssetPrice[], AssetError>> {
    return commands.getAssetPrices(assetId);
  },

  async updateAssetPrice(
    assetId: string,
    originalDate: string,
    newDate: string,
    newPrice: number,
  ): Promise<Result<null, AssetError>> {
    return commands.updateAssetPrice(assetId, originalDate, newDate, newPrice);
  },

  async deleteAssetPrice(assetId: string, date: string): Promise<Result<null, AssetError>> {
    return commands.deleteAssetPrice(assetId, date);
  },

  async openHolding(dto: OpenHoldingDTO): Promise<Result<Transaction, OpenHoldingError>> {
    return commands.openHolding(dto);
  },

  async recordDeposit(dto: DepositDTO): Promise<Result<Transaction, AccountError>> {
    return commands.recordDeposit(dto);
  },

  async recordWithdrawal(dto: WithdrawalDTO): Promise<Result<Transaction, AccountError>> {
    return commands.recordWithdrawal(dto);
  },

  async recordDividend(dto: DividendDTO): Promise<Result<Transaction, DividendError>> {
    return commands.recordDividend(dto);
  },

  // FSD-022 — record a zero-cost free-share distribution attributed to a held asset.
  async recordFreeShares(dto: FreeSharesDTO): Promise<Result<Transaction, FreeSharesError>> {
    return commands.recordFreeShares(dto);
  },

  // SPL-010/020 — record a stock split rescaling a held position at its date.
  async recordSplit(dto: RecordSplitDTO): Promise<Result<Transaction, SplitError>> {
    return commands.recordSplit(dto);
  },

  // INT-023/024 — record a zero-cost interest credit on a held asset or the cash line.
  async recordInterest(dto: RecordInterestDTO): Promise<Result<Transaction, InterestError>> {
    return commands.recordInterest(dto);
  },

  // FEE-022/028 — record a one-off management fee, as a percentage of the held quantity
  // or as the quantity the holding should end at.
  async recordManagementFee(
    dto: ManagementFeeDTO,
  ): Promise<Result<Transaction, ManagementFeeError>> {
    return commands.recordManagementFee(dto);
  },

  // FEE-029 — what a one-off fee entered by its resulting quantity would remove.
  async previewManagementFee(
    accountId: string,
    assetId: string,
    date: string,
    resultingQuantityMicros: number,
  ): Promise<Result<ManagementFeeRemoval, ManagementFeeError>> {
    return commands.previewManagementFee(accountId, assetId, date, resultingQuantityMicros);
  },

  // FEE-030 — create the recurring fee schedule for an (account, asset) pair.
  async createFeeSchedule(dto: CreateFeeScheduleDTO): Promise<Result<FeeSchedule, AccountError>> {
    return commands.createFeeSchedule(dto);
  },

  // FEE-060/061 — edit the rate / end date / active flag of an existing schedule.
  async updateFeeSchedule(dto: UpdateFeeScheduleDTO): Promise<Result<FeeSchedule, AccountError>> {
    return commands.updateFeeSchedule(dto);
  },

  // FEE-062 — remove the schedule for an (account, asset) pair.
  async deleteFeeSchedule(accountId: string, assetId: string): Promise<Result<null, AccountError>> {
    return commands.deleteFeeSchedule(accountId, assetId);
  },

  // FEE-030 — load the existing schedule for an (account, asset) pair, or null.
  async getFeeSchedule(
    accountId: string,
    assetId: string,
  ): Promise<Result<FeeSchedule | null, AccountError>> {
    return commands.getFeeSchedule(accountId, assetId);
  },

  // HNO-020 — create or fully replace the note pinned to an (account, asset) pair.
  async upsertHoldingNote(dto: UpsertHoldingNoteDTO): Promise<Result<HoldingNote, AccountError>> {
    return commands.upsertHoldingNote(dto);
  },

  // HNO-021 — delete the note for an (account, asset) pair (no-op success when absent).
  async deleteHoldingNote(dto: DeleteHoldingNoteDTO): Promise<Result<null, AccountError>> {
    return commands.deleteHoldingNote(dto);
  },

  // CSH-111 — editing a cash Deposit/Withdrawal persists via correct_transaction.
  async correctTransaction(
    id: string,
    accountId: string,
    dto: CorrectTransactionFields,
  ): Promise<Result<Transaction, AccountError>> {
    return commands.correctTransaction({ ...dto, account_id: accountId, transaction_id: id });
  },

  // TDI-010 — holding quantity + VWAP average cost as of a date (trade-dialog insights).
  async getHoldingSnapshotAsOf(
    accountId: string,
    assetId: string,
    date: string,
  ): Promise<Result<HoldingSnapshot, AccountError>> {
    return commands.getHoldingSnapshotAsOf(accountId, assetId, date);
  },

  async fetchAccountAssetPrices(
    accountId: string,
  ): Promise<Result<null, FetchAccountAssetPricesError>> {
    return commands.fetchAccountAssetPrices(accountId);
  },

  async blockAssetPriceRefresh(assetId: string): Promise<Result<null, AssetError>> {
    return commands.blockAssetPriceRefresh(assetId);
  },

  async unblockAssetPriceRefresh(assetId: string): Promise<Result<null, AssetError>> {
    return commands.unblockAssetPriceRefresh(assetId);
  },

  // MKT-190 — fill the holding's price history over the period the account held it.
  async backfillHoldingPriceHistory(
    accountId: string,
    assetId: string,
  ): Promise<Result<PriceHistoryBackfillOutcome, PriceHistoryBackfillError>> {
    return commands.backfillHoldingPriceHistory(accountId, assetId);
  },

  async subscribeToEvents(callback: (type: Event["type"]) => void): Promise<() => void> {
    return events.event.listen((event) => {
      callback(event.payload.type);
    });
  },
};

/**
 * Asset catalog from the shared BE/FE cache (F28). Feature code reads the cache
 * through its own gateway rather than importing the store directly.
 */
export function useCachedAssets() {
  return useAppStore((state) => state.assets);
}

/** Account list from the shared BE/FE cache (F28). */
export function useCachedAccounts() {
  return useAppStore((state) => state.accounts);
}
