import { create } from "zustand";
import type { Exchange } from "@/bindings";
import { assetGateway } from "./gateway";

interface AssetsState {
  supportedExchanges: Exchange[];
  loadSupportedExchanges: () => Promise<void>;
}

export const useAssetsStore = create<AssetsState>((set, get) => ({
  supportedExchanges: [],
  loadSupportedExchanges: async () => {
    if (get().supportedExchanges.length > 0) return;
    const exchanges = await assetGateway.getSupportedExchanges();
    set({ supportedExchanges: exchanges });
  },
}));
