import { useEffect, useState } from "react";
import type { WrapperLists } from "@/bindings";
import { logger } from "@/lib/logger";
import { accountGateway } from "./gateway";

// ACC-040 — the lists never change while the application runs: read once, the request
// itself shared, so views mounted together ask once.
let cached: WrapperLists | null = null;
let reading: Promise<WrapperLists | null> | null = null;

function readOnce(): Promise<WrapperLists | null> {
  reading ??= accountGateway
    .getWrapperLists()
    .then((read) => {
      cached = read;
      return read;
    })
    .catch((error) => {
      // The fields stay hidden; a later view asks again.
      logger.error("[useWrapperLists] reading the wrapper lists failed", { error });
      reading = null;
      return null;
    });
  return reading;
}

/** ACC-045 — the wrapper lists, or null until they are read (or when they cannot be). */
export function useWrapperLists(): WrapperLists | null {
  const [lists, setLists] = useState<WrapperLists | null>(cached);
  useEffect(() => {
    if (cached) return;
    let mounted = true;
    readOnce().then((read) => {
      if (mounted && read) setLists(read);
    });
    return () => {
      mounted = false;
    };
  }, []);
  return lists;
}
