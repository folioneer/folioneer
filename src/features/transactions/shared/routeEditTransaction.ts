import type { Transaction } from "@/bindings";
import { patchModalSearch } from "@/lib/modalSearch";

/**
 * Routes a transaction edit to the correct surface (TRX-036). Cash Deposit/Withdrawal,
 * FreeShares, ManagementFee, Split, and Interest corrections open their dedicated
 * modals via the URL-driven modal mount (the generic modal is cash-excluded,
 * CSH-018 / FSD-040 / FEE-063 / SPL-030 / INT-040); everything else opens the generic
 * edit modal through `openGenericModal`.
 * Shared by the per-asset and account-wide journals so the branching lives in one place.
 */
export function routeEditTransaction(
  navigate: Parameters<typeof patchModalSearch>[0],
  raw: Transaction,
  openGenericModal: (transaction: Transaction) => void,
): void {
  switch (raw.transaction_type) {
    case "Deposit":
      patchModalSearch(navigate, {
        modal: "edit-cash-deposit",
        editTxId: raw.id,
        editTxAccountId: raw.account_id,
        editTxAssetId: raw.asset_id,
      });
      break;
    case "Withdrawal":
      patchModalSearch(navigate, {
        modal: "edit-cash-withdrawal",
        editTxId: raw.id,
        editTxAccountId: raw.account_id,
        editTxAssetId: raw.asset_id,
      });
      break;
    case "FreeShares":
      patchModalSearch(navigate, {
        modal: "edit-free-shares",
        editTxId: raw.id,
        editTxAccountId: raw.account_id,
        editTxAssetId: raw.asset_id,
      });
      break;
    case "ManagementFee":
      patchModalSearch(navigate, {
        modal: "edit-management-fee",
        editTxId: raw.id,
        editTxAccountId: raw.account_id,
        editTxAssetId: raw.asset_id,
      });
      break;
    case "Split":
      patchModalSearch(navigate, {
        modal: "edit-split",
        editTxId: raw.id,
        editTxAccountId: raw.account_id,
        editTxAssetId: raw.asset_id,
      });
      break;
    case "Interest":
      patchModalSearch(navigate, {
        modal: "edit-interest",
        editTxId: raw.id,
        editTxAccountId: raw.account_id,
        editTxAssetId: raw.asset_id,
      });
      break;
    default:
      openGenericModal(raw);
  }
}
