import { useEffect, useState } from "react";
import { logger } from "@/lib/logger";
import { listProfiles, onProfilesChanged } from "./gateway";

/**
 * #768 — how many profiles there are, for the sidebar's "Profiles" entry; read again when the
 * profiles change (PFL-034) or a sync applied changes. 0 until read, or when they cannot be.
 */
export function useProfileCount(): number {
  const [count, setCount] = useState(0);

  useEffect(() => {
    let isMounted = true;
    const load = async () => {
      const result = await listProfiles();
      if (!isMounted) return;
      if (result.status === "error") {
        logger.error("[useProfileCount] the profiles could not be read", {
          code: result.error.code,
        });
        return;
      }
      setCount(result.data.profiles.length);
    };
    void load();
    const unlisten = onProfilesChanged(() => void load());
    return () => {
      isMounted = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  return count;
}
