import { useEffect, useState } from "react";
import type { ProfileList } from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { listProfiles, onProfilesChanged } from "./gateway";
import { profileErrorToI18n } from "./shared/presenter";

/**
 * #768 / #770 — the profiles and the one in use, for the sidebar's "Profiles" entry and the
 * agent's connection dialog; read again when the profiles change (PFL-034) or a sync applied
 * changes. `list` is null until read, or when they cannot be; `error` then says why (F27).
 */
export function useProfileList(): { list: ProfileList | null; error: I18nMessage | null } {
  const [list, setList] = useState<ProfileList | null>(null);
  const [error, setError] = useState<I18nMessage | null>(null);

  useEffect(() => {
    let isMounted = true;
    const load = async () => {
      const result = await listProfiles();
      if (!isMounted) return;
      if (result.status === "error") {
        logger.error("[useProfileList] the profiles could not be read", {
          code: result.error.code,
        });
        setError(profileErrorToI18n(result.error));
        return;
      }
      setError(null);
      setList(result.data);
    };
    void load();
    const unlisten = onProfilesChanged(() => void load());
    return () => {
      isMounted = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  return { list, error };
}
