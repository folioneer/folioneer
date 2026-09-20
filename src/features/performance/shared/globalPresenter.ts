import type { Account, Asset } from "@/bindings";
import { isCashAsset } from "@/lib/cashAsset";
import type { AssetScopeOption } from "./presenter";

/** One selectable account scope in the global-performance account selector. */
export interface AccountScopeOption {
  accountId: string;
  accountName: string;
}

/** The account scopes offerable on the global-performance page, name asc. */
export function presentAccountScopeOptions(accounts: Account[]): AccountScopeOption[] {
  return accounts
    .map((account) => ({ accountId: account.id, accountName: account.name }))
    .sort((a, b) => a.accountName.localeCompare(b.accountName));
}

/**
 * The asset scopes offerable when every account is in scope: the active
 * (non-archived) non-cash assets of the catalog, name asc — mirroring the
 * per-account holdings order (asset_name asc) and the PRF-082 cash exclusion.
 */
export function presentAssetCatalogOptions(assets: Asset[]): AssetScopeOption[] {
  return assets
    .filter((asset) => !asset.is_archived && !isCashAsset(asset.id))
    .map((asset) => ({ assetId: asset.id, assetName: asset.name }))
    .sort((a, b) => a.assetName.localeCompare(b.assetName));
}
