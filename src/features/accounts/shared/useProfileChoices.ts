import { useEffect, useState } from "react";
import type { ProfileList } from "@/bindings";
import { logger } from "@/lib/logger";
import { accountGateway } from "../gateway";

/**
 * PFL-021 / PFL-031 — the profiles an account may belong to and the profile in use, for the
 * account form; read again on `ProfileUpdated`. `null` until read, or when they cannot be.
 */
export function useProfileChoices(): ProfileList | null {
  const [list, setList] = useState<ProfileList | null>(null);

  useEffect(() => {
    let isMounted = true;
    const load = async () => {
      const result = await accountGateway.listProfiles();
      if (!isMounted) return;
      if (result.status === "error") {
        logger.error("[useProfileChoices] the profiles could not be read", {
          code: result.error.code,
        });
        return;
      }
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

  return list;
}
