import { type AssetCategory, type AssetError, commands, type Result } from "@/bindings";

export const categoryGateway = {
  async getCategories(): Promise<Result<AssetCategory[], AssetError>> {
    return commands.getCategories();
  },

  async addCategory(label: string): Promise<Result<AssetCategory, AssetError>> {
    return commands.addCategory(label);
  },

  async updateCategory(id: string, label: string): Promise<Result<AssetCategory, AssetError>> {
    return commands.updateCategory(id, label);
  },

  async deleteCategory(id: string): Promise<Result<null, AssetError>> {
    return commands.deleteCategory(id);
  },
};
