import { useEffect, useState } from "react";
import type { ProfileList } from "@/bindings";
import { logger } from "@/lib/logger";
import { listProfiles, onProfilesChanged } from "./gateway";

/**
 * #768 / #770 — the profiles and the one in use, for the sidebar's "Profiles" entry and the
 * agent's connection dialog; read again when the profiles change (PFL-034) or a sync applied
 * changes. null until read, or when they cannot be.
 */
export function useProfileList(): ProfileList | null {
  const [list, setList] = useState<ProfileList | null>(null);

  useEffect(() => {
    let isMounted = true;
    const load = async () => {
      const result = await listProfiles();
      if (!isMounted) return;
      if (result.status === "error") {
        logger.error("[useProfileList] the profiles could not be read", {
          code: result.error.code,
        });
        return;
      }
      setList(result.data);
    };
    void load();
    const unlisten = onProfilesChanged(() => void load());
    return () => {
      isMounted = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  return list;
}
