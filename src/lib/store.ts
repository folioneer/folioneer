import { getName, getVersion } from "@tauri-apps/api/app";
import i18next from "i18next";
import { create } from "zustand";
import {
  type Account,
  type Asset,
  type AssetCategory,
  events,
  type UnpricedAsset,
} from "../bindings";
import { accountGateway } from "../features/accounts/gateway";
import { assetGateway } from "../features/assets/gateway";
import { categoryGateway } from "../features/categories/gateway";
import { getCapabilities } from "../features/shell/gateway";
import { type SnackbarVariant, useSnackbarStore } from "../ui/components/snackbar/snackbarStore";
import { logger } from "./logger";

/**
 * Builds the snackbar for a finished price-fetch task (MKT-145), or `null` when
 * the task fully succeeded (`skipped === 0`) and nothing should be shown.
 */
export function buildPriceFetchFeedback(
  ok: number,
  skipped: number,
): { message: string; variant: SnackbarVariant } | null {
  if (skipped === 0) return null;
  return ok > 0
    ? { message: i18next.t("mkt.fetch_completed_partial", { ok, skipped }), variant: "info" }
    : { message: i18next.t("mkt.fetch_completed_failed", { skipped }), variant: "error" };
}

interface AppState {
  // Application metadata
  appName: string;
  appVersion: string;

  // Data
  assets: Asset[];
  categories: AssetCategory[];
  accounts: Account[];

  // Assets a fetch task could not price (MKT-170); drives the manual-fill modal.
  unpricedAssets: UnpricedAsset[];

  // MKT-180 — in-flight price-fetch progress; drives the shell progress bar and
  // the MKT-181 re-fetch coalescing in the view hooks.
  priceFetch: { active: boolean; done: number; total: number };
  // MKT-211 — whether this build has an External provider; read before the router mounts.
  hasExternalProvider: boolean;

  // Loading states
  isLoadingAssets: boolean;
  isLoadingCategories: boolean;
  isLoadingAccounts: boolean;
  isInitialized: boolean;

  // Error handling
  assetsError: string | null;
  categoriesError: string | null;
  accountsError: string | null;

  // Actions
  fetchAssets: () => Promise<void>;
  fetchCategories: () => Promise<void>;
  fetchAccounts: () => Promise<void>;

  // MKT-177 — dismiss the manual-fill modal, clearing the unpriced list.
  clearUnpricedAssets: () => void;

  // Initialization
  isAnyLoading: () => boolean;
  init: () => () => void;
}

/**
 * MKT-172 / PMV-018 — whether the manual-fill modal holds the screen. The one
 * definition its shell mount and every other reader consume.
 */
export const selectUnpricedModalOpen = (state: AppState): boolean =>
  state.unpricedAssets.length > 0;

/**
 * MKT-212 — whether the interface may offer what needs an External provider: a fetch
 * task, a price history backfill, the price refresh lock, the scheduled fetch.
 */
export const selectHasExternalProvider = (state: AppState): boolean => state.hasExternalProvider;

export const useAppStore = create<AppState>((set, get) => {
  return {
    appName: "Folioneer",
    appVersion: "...",
    assets: [],
    categories: [],
    accounts: [],
    unpricedAssets: [],
    priceFetch: { active: false, done: 0, total: 0 },
    // Every build has had an External provider until now; `init` reads the real answer
    // before `isInitialized` flips, so no screen renders from this default.
    hasExternalProvider: true,
    isLoadingAssets: false,
    isLoadingCategories: false,
    isLoadingAccounts: false,
    isInitialized: false,
    assetsError: null,
    categoriesError: null,
    accountsError: null,

    fetchAssets: async () => {
      set({ isLoadingAssets: true, assetsError: null });
      const result = await assetGateway.getAssetsWithArchived();
      if (result.status === "ok") {
        set({ assets: result.data, isLoadingAssets: false });
      } else {
        set({
          assetsError: `error.${result.error.code}`,
          isLoadingAssets: false,
        });
      }
    },

    fetchCategories: async () => {
      set({ isLoadingCategories: true, categoriesError: null });
      const result = await categoryGateway.getCategories();
      if (result.status === "ok") {
        set({ categories: result.data, isLoadingCategories: false });
      } else {
        set({
          categoriesError: `error.${result.error.code}`,
          isLoadingCategories: false,
        });
      }
    },

    fetchAccounts: async () => {
      set({ isLoadingAccounts: true, accountsError: null });
      const result = await accountGateway.getAccounts();
      if (result.status === "ok") {
        set({ accounts: result.data, isLoadingAccounts: false });
      } else {
        set({
          accountsError: `error.${result.error.code}`,
          isLoadingAccounts: false,
        });
      }
    },

    clearUnpricedAssets: () => set({ unpricedAssets: [] }),

    isAnyLoading: () => {
      const state = get();
      return state.isLoadingAssets || state.isLoadingCategories || state.isLoadingAccounts;
    },

    init: () => {
      if (get().isInitialized) {
        return () => {};
      }

      const { fetchAssets, fetchCategories, fetchAccounts } = get();

      const fetchMetadata = async () => {
        try {
          const [name, version] = await Promise.all([getName(), getVersion()]);
          set({ appName: name, appVersion: version });
        } catch (e) {
          logger.error("[store] failed to fetch app metadata", e);
        }
      };

      // MKT-211 — settled for the run; a failure keeps the default and is logged.
      const fetchCapabilities = async () => {
        try {
          const capabilities = await getCapabilities();
          set({ hasExternalProvider: capabilities.external_provider });
        } catch (e) {
          logger.error("[store] failed to read the build's capabilities", e);
        }
      };

      // initial parallelized fetch
      Promise.all([
        fetchAssets(),
        fetchCategories(),
        fetchAccounts(),
        fetchMetadata(),
        fetchCapabilities(),
      ]).then(() => {
        set({ isInitialized: true });
      });

      // Bus Event
      const eventMap: Record<string, () => void> = {
        AssetUpdated: fetchAssets,
        CategoryUpdated: fetchCategories,
        AccountUpdated: fetchAccounts,
        // SYN-064 — a bare marker: a run applied other devices' changes, so every
        // cached slice is re-read.
        SyncCompleted: () => {
          fetchAssets();
          fetchCategories();
          fetchAccounts();
        },
      };

      // Events handled locally by feature hooks (e.g. useAccountDetails) — not global store concerns
      const locallyHandledEvents = new Set([
        "TransactionUpdated",
        "CurrencyRateUpdated",
        "CurrencyPairUpdated",
        "HoldingNoteUpdated",
      ]);

      // Setup event listeners
      const unlistenPromise = events.event.listen((event) => {
        const payload = event.payload;
        // The global store is the app's single always-on event sink, so a
        // launch-time fetch-failure snackbar surfaces regardless of the open view.
        // MKT-180 — track in-flight fetch progress for the shell progress bar.
        if (payload.type === "AssetPriceFetchProgress") {
          set({ priceFetch: { active: true, done: payload.done, total: payload.total } });
          return;
        }
        if (payload.type === "AssetPriceFetchCompleted") {
          // MKT-180 — the task is over; hide the progress bar.
          set({ priceFetch: { active: false, done: 0, total: 0 } });
          // MKT-172/173 — each completed fetch refreshes the unpriced list. A
          // non-empty list opens the manual-fill modal and supersedes the MKT-145
          // snackbar; an empty list clears any stale modal and lets the snackbar
          // surface the outcome as before.
          set({ unpricedAssets: payload.unpriced });
          if (payload.unpriced.length === 0) {
            const feedback = buildPriceFetchFeedback(payload.ok, payload.skipped);
            if (feedback) {
              useSnackbarStore.getState().show(feedback.message, feedback.variant);
            }
          }
          return;
        }
        const handler = eventMap[payload.type];
        if (handler) {
          handler();
        } else if (!locallyHandledEvents.has(payload.type)) {
          logger.debug("[store] unhandled event", { type: payload.type });
        }
      });

      // Return cleanup function
      return () => {
        unlistenPromise.then((unlisten) => unlisten());
        set({ isInitialized: false });
      };
    },
  };
});
