import {
  type ArchiveAssetError,
  type Asset,
  type AssetError,
  type AssetLookupResult,
  type CreateAssetDTO,
  commands,
  type DeleteAssetError,
  type Exchange,
  type LookupMode,
  type Result,
  type UpdateAssetDTO,
  type WebLookupError,
} from "../../bindings";

/**
 * Gateway for Asset-related backend communication.
 * Centralizes all Tauri command calls for the Asset feature.
 */
export const assetGateway = {
  async getAssets(): Promise<Result<Asset[], AssetError>> {
    return await commands.getAssets();
  },

  async getAssetsWithArchived(): Promise<Result<Asset[], AssetError>> {
    return await commands.getAssetsWithArchived();
  },

  async createAsset(dto: CreateAssetDTO): Promise<Result<Asset, AssetError>> {
    return await commands.addAsset(dto);
  },

  async updateAsset(dto: UpdateAssetDTO): Promise<Result<Asset, AssetError>> {
    return await commands.updateAsset(dto);
  },

  async archiveAsset(id: string): Promise<Result<null, ArchiveAssetError>> {
    return await commands.archiveAsset(id);
  },

  async unarchiveAsset(id: string): Promise<Result<null, AssetError>> {
    return await commands.unarchiveAsset(id);
  },

  async deleteAsset(id: string): Promise<Result<null, DeleteAssetError>> {
    return await commands.deleteAsset(id);
  },

  async lookupAsset(
    query: string,
    mode: LookupMode,
  ): Promise<Result<AssetLookupResult[], WebLookupError>> {
    return await commands.lookupAsset(query, mode);
  },

  async getSupportedExchanges(): Promise<Exchange[]> {
    return await commands.getSupportedExchanges();
  },
};
