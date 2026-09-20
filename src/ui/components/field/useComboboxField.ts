import { useState } from "react";
import { useFuzzySearch } from "@/lib/useFuzzySearch";

/**
 * useComboboxField - Logic for the generic ComboboxField component.
 *
 * Manages the text query and the fuzzy-filtered suggestions.
 */
export function useComboboxField<T extends object>(
  items: T[],
  displayKey: keyof T,
  searchKeys?: (keyof T)[],
) {
  const [query, setQuery] = useState("");

  const keys = searchKeys ? searchKeys.map(String) : [String(displayKey)];
  const fuzzyResults = useFuzzySearch(query, items, keys);
  // Fuzzy search only kicks in at 2 characters; below that, offer the full list
  // so the field reads as a browsable dropdown rather than a readonly input.
  const filteredItems = query.length >= 2 ? fuzzyResults : items;

  return { query, setQuery, filteredItems };
}
