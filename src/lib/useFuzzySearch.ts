import Fuse from "fuse.js";
import { useMemo } from "react";

/**
 * Generic hook to handle fuzzy search logic using Fuse.js
 */
export function useFuzzySearch<T>(query: string, list: T[], keys: string[], threshold = 0.3) {
  const fuse = useMemo(() => {
    return new Fuse(list, {
      keys,
      threshold,
      distance: 100,
    });
  }, [list, keys, threshold]);

  return useMemo(() => {
    // We only start searching after 2 characters for better performance
    if (query.length < 2) {
      return [];
    }

    // Execute search and extract the items from the results
    return fuse.search(query).map((result) => result.item);
  }, [query, fuse]);
}
