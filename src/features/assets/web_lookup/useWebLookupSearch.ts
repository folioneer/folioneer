import { useCallback, useState } from "react";
import type { AssetLookupResult, LookupMode, WebLookupError } from "@/bindings";
import { assetGateway } from "../gateway";

export type WebLookupSearchState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "results"; results: AssetLookupResult[] }
  | { status: "empty" }
  | { status: "error"; code: WebLookupError["code"] };

export interface UseWebLookupSearchReturn {
  query: string;
  setQuery: (q: string) => void;
  state: WebLookupSearchState;
  lastMode: LookupMode | null;
  /**
   * Dispatches a search. When `queryOverride` is provided it takes precedence
   * over the internal `query` state — required by the two-field modal where
   * the internal state mirrors only the most-recently-typed field, not the
   * one whose submit button was actually clicked.
   */
  submit: (mode: LookupMode, queryOverride?: string) => void;
  retry: () => void;
}

export function useWebLookupSearch(): UseWebLookupSearchReturn {
  const [query, setQuery] = useState("");
  const [state, setState] = useState<WebLookupSearchState>({ status: "idle" });
  const [lastMode, setLastMode] = useState<LookupMode | null>(null);
  const [lastQuery, setLastQuery] = useState<string>("");

  const runSearch = useCallback(async (q: string, mode: LookupMode) => {
    setLastMode(mode);
    setLastQuery(q);
    setState({ status: "loading" });
    const result = await assetGateway.lookupAsset(q, mode);
    if (result.status === "error") {
      setState({ status: "error", code: result.error.code });
    } else if (result.data.length === 0) {
      setState({ status: "empty" });
    } else {
      setState({ status: "results", results: result.data });
    }
  }, []);

  const submit = useCallback(
    (mode: LookupMode, queryOverride?: string) => {
      const effective = queryOverride ?? query;
      if (!effective.trim() || state.status === "loading") return;
      runSearch(effective, mode);
    },
    [query, state, runSearch],
  );

  const retry = useCallback(() => {
    if (!lastQuery.trim() || state.status === "loading" || !lastMode) return;
    runSearch(lastQuery, lastMode);
  }, [lastQuery, state, lastMode, runSearch]);

  return { query, setQuery, state, lastMode, submit, retry };
}
