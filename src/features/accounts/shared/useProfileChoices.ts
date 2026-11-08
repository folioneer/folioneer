import { useEffect, useState } from "react";
import type { ProfileList } from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountGateway } from "../gateway";
import { accountMutationErrorToI18n } from "./presenter";

/**
 * PFL-021 / PFL-031 — the profiles an account may belong to and the profile in use, for the
 * account form; read again on `ProfileUpdated`. `list` is `null` until read; a refused read is
 * kept as `error` for the form to show (F27).
 */
export function useProfileChoices(): { list: ProfileList | null; error: I18nMessage | null } {
  const [list, setList] = useState<ProfileList | null>(null);
  const [error, setError] = useState<I18nMessage | null>(null);

  useEffect(() => {
    let isMounted = true;
    const load = async () => {
      const result = await accountGateway.listProfiles();
      if (!isMounted) return;
      if (result.status === "error") {
        logger.error("[useProfileChoices] the profiles could not be read", {
          code: result.error.code,
        });
        setError(accountMutationErrorToI18n(result.error));
        return;
      }
      setError(null);
      setList(result.data);
    };
    void load();
    const unlisten = accountGateway.subscribeToEvents((type) => {
      if (type === "ProfileUpdated" || type === "SyncCompleted") void load();
    });
    return () => {
      isMounted = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  return { list, error };
}
