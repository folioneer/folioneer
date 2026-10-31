import { useCallback, useEffect, useState } from "react";
import type { PriceProviderChoice } from "@/bindings";
import { logger } from "@/lib/logger";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";
import { fetchAllAssetPrices, getPriceProviderChoice, setPriceProviderChoice } from "../gateway";
import { priceProviderChoiceErrorToI18n } from "../shared/presenter";

export const PROVIDER_OFF = "off";

export interface UsePriceProviderResult {
  /** The choice and what the build offers; null until read, or when the read failed. */
  choice: PriceProviderChoice | null;
  /** What the list shows: the source awaiting its notice's acknowledgement, or the choice. */
  selected: string;
  /** The source whose notice is shown; it is not the choice yet. */
  awaitingAcknowledgement: string | null;
  isLoading: boolean;
  /** MKT-216 — true while a choice is being saved. */
  isSaving: boolean;
  /** A failed read or a refused choice; the choice stays as it was. */
  error: I18nMessage | null;
  select: (source: string) => void;
  acknowledge: () => void;
}

/** Fire-and-forget, as the launch's auto-fetch: the shell shows its progress, a failure is logged. */
async function startFetching(): Promise<void> {
  try {
    const result = await fetchAllAssetPrices();
    if (result.status === "error") {
      logger.warn("[usePriceProvider] the first fetch was refused", { code: result.error.code });
    }
  } catch (error) {
    logger.error("[usePriceProvider] the first fetch threw", { error });
  }
}

/**
 * MKT-215 — the choice of a price source on this computer: choosing a provider asks for its
 * notice's acknowledgement before anything is saved; Off is saved at once. A saved choice
 * makes the interface ask again what it was told (MKT-217), and a source turned on starts
 * fetching (MKT-218).
 */
export function usePriceProvider(): UsePriceProviderResult {
  const refreshExternalProvider = useAppStore((state) => state.refreshExternalProvider);
  const [choice, setChoice] = useState<PriceProviderChoice | null>(null);
  const [awaitingAcknowledgement, setAwaitingAcknowledgement] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<I18nMessage | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const read = await getPriceProviderChoice();
        if (!cancelled) setChoice(read);
      } catch (e) {
        logger.error("[usePriceProvider] failed to read the price source", e);
        if (!cancelled) setError({ key: "settings.price_provider_load_failed" });
      } finally {
        if (!cancelled) setIsLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const save = useCallback(
    async (chosen: string | null) => {
      setIsSaving(true);
      setError(null);
      try {
        const result = await setPriceProviderChoice(chosen);
        if (result.status === "error") {
          setError(priceProviderChoiceErrorToI18n(result.error));
          return;
        }
        setChoice(result.data);
        setAwaitingAcknowledgement(null);
        await refreshExternalProvider();
        if (chosen !== null) void startFetching();
      } catch (e) {
        logger.error("[usePriceProvider] failed to save the choice", e);
        setError({ key: "error.price_provider.NotSaved" });
      } finally {
        setIsSaving(false);
      }
    },
    [refreshExternalProvider],
  );

  const select = useCallback(
    (source: string) => {
      setError(null);
      if (source === (choice?.chosen ?? PROVIDER_OFF)) {
        setAwaitingAcknowledgement(null);
      } else if (source === PROVIDER_OFF) {
        setAwaitingAcknowledgement(null);
        void save(null);
      } else {
        setAwaitingAcknowledgement(source);
      }
    },
    [choice, save],
  );

  const acknowledge = useCallback(() => {
    if (awaitingAcknowledgement !== null) void save(awaitingAcknowledgement);
  }, [awaitingAcknowledgement, save]);

  return {
    choice,
    selected: awaitingAcknowledgement ?? choice?.chosen ?? PROVIDER_OFF,
    awaitingAcknowledgement,
    isLoading,
    isSaving,
    error,
    select,
    acknowledge,
  };
}
