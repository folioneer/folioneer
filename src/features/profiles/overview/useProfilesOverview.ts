import { useNavigate } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import type { ProfileOverview } from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { profileGateway } from "../gateway";
import { profileErrorToI18n } from "../shared/presenter";

/**
 * PFL-055 — every profile with its own figures, read on mount and again whenever a profile,
 * an account, a transaction, a price or a rate changes, or a sync applied changes.
 */
export function useProfilesOverview() {
  const navigate = useNavigate();
  const [overview, setOverview] = useState<ProfileOverview | null>(null);
  const [error, setError] = useState<I18nMessage | null>(null);

  const load = useCallback(async () => {
    const result = await profileGateway.getProfileOverview();
    if (result.status === "error") {
      logger.error("[useProfilesOverview] the overview could not be read", {
        code: result.error.code,
      });
      setError(profileErrorToI18n(result.error));
      return;
    }
    setError(null);
    setOverview(result.data);
  }, []);

  useEffect(() => {
    let isMounted = true;
    void load();
    const unlisten = profileGateway.subscribeToEvents((type) => {
      if (!isMounted) return;
      if (
        type === "ProfileUpdated" ||
        type === "AccountUpdated" ||
        type === "TransactionUpdated" ||
        type === "AssetPriceUpdated" ||
        type === "CurrencyRateUpdated" ||
        type === "ReferenceCurrencyUpdated" ||
        type === "SyncCompleted"
      ) {
        void load();
      }
    });
    return () => {
      isMounted = false;
      void unlisten.then((stop) => stop());
    };
  }, [load]);

  // #768 (the validated mock) — a click makes the profile the one in use and opens its accounts.
  const open = async (profileId: string) => {
    if (overview?.in_use !== profileId) {
      const result = await profileGateway.setProfileInUse(profileId);
      if (result.status === "error") {
        logger.warn("[useProfilesOverview] the profile was not chosen", {
          code: result.error.code,
        });
        setError(profileErrorToI18n(result.error));
        return;
      }
    }
    void navigate({ to: "/accounts" });
  };

  return { overview, error, open };
}
