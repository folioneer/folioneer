import { create } from "zustand";
import type { AssetLookupResult } from "@/bindings";

export interface AssetCreationRequest {
  prefill: AssetLookupResult;
  onCreated: (assetId: string) => void;
}

interface AssetCreationState {
  request: AssetCreationRequest | null;
  open: (request: AssetCreationRequest) => void;
  close: () => void;
}

/**
 * IMP-052 — another feature's request for the usual asset form, prefilled, and for the id of
 * the asset created. The shell hosts the form (`AssetCreationMount`, F26): a request holds a
 * callback, which a modal URL parameter could not.
 */
export const useAssetCreationRequest = create<AssetCreationState>((set) => ({
  request: null,
  open: (request) => set({ request }),
  close: () => set({ request: null }),
}));
