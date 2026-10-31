import { useCallback, useEffect, useState } from "react";
import type { PriceProviderChoice } from "@/bindings";
import { logger } from "@/lib/logger";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";
import { fetchPricesOnChoosingSource, getPriceSource, setPriceSource } from "../gateway";
import { priceProviderChoiceErrorToI18n } from "../shared/presenter";

export const SOURCE_OFF = "off";

export interface UsePriceSourceResult {
  /** The choice and what the build offers; null until read, or when the read failed. */
  choice: PriceProviderChoice | null;
  /** What the list shows: the source awaiting its notice's acknowledgement, or the choice. */
  selected: string;
  /** The source whose notice is shown; it is not the choice yet. */
  awaitingAcknowledgement: string | null;
  isLoading: boolean;
  /** True while a choice is being saved. */
  isSaving: boolean;
  /** A failed read or a refused choice; the choice stays as it was. */
  error: I18nMessage | null;
  select: (source: string) => void;
  acknowledge: () => void;
}

/**
 * MKT-215 — the choice of a price source on this computer: choosing a provider asks for its
 * notice's acknowledgement before anything is saved; Off is saved at once. A saved choice
 * makes the interface ask again what it was told (MKT-211), and a source turned on starts
 * fetching.
 */
export function usePriceSource(): UsePriceSourceResult {
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
        const read = await getPriceSource();
        if (!cancelled) setChoice(read);
      } catch (e) {
        logger.error("[usePriceSource] failed to read the price source", e);
        if (!cancelled) setError({ key: "settings.price_source_load_failed" });
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
        const result = await setPriceSource(chosen);
        if (result.status === "error") {
          setError(priceProviderChoiceErrorToI18n(result.error));
          return;
        }
        setChoice(result.data);
        setAwaitingAcknowledgement(null);
        await refreshExternalProvider();
        if (chosen !== null) void fetchPricesOnChoosingSource();
      } finally {
        setIsSaving(false);
      }
    },
    [refreshExternalProvider],
  );

  const select = useCallback(
    (source: string) => {
      setError(null);
      if (source === (choice?.chosen ?? SOURCE_OFF)) {
        setAwaitingAcknowledgement(null);
      } else if (source === SOURCE_OFF) {
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
    selected: awaitingAcknowledgement ?? choice?.chosen ?? SOURCE_OFF,
    awaitingAcknowledgement,
    isLoading,
    isSaving,
    error,
    select,
    acknowledge,
  };
}
