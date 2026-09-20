import { useCallback, useState } from "react";
import type { AssetLookupResult, LookupMode } from "@/bindings";
import type { WebLookupSearchState } from "./useWebLookupSearch";
import { useWebLookupSearch } from "./useWebLookupSearch";

export type ModalStep =
  | { step: "search" }
  | { step: "form-prefilled"; selection: AssetLookupResult }
  | { step: "form-manual" };

export interface UseWebLookupModalReturn {
  modalStep: ModalStep;
  searchState: WebLookupSearchState;
  isinQuery: string;
  keywordQuery: string;
  lastMode: LookupMode | null;
  setIsinQuery: (q: string) => void;
  setKeywordQuery: (q: string) => void;
  submitSearch: (mode: LookupMode) => void;
  retrySearch: () => void;
  selectResult: (result: AssetLookupResult) => void;
  fillManually: () => void;
  back: () => void;
  canGoBack: boolean;
}

export function useWebLookupModal(): UseWebLookupModalReturn {
  const search = useWebLookupSearch();
  const [isinQuery, setIsinQueryState] = useState("");
  const [keywordQuery, setKeywordQueryState] = useState("");
  const [modalStep, setModalStep] = useState<ModalStep>({ step: "search" });

  // reviewer-frontend FP: `[search]` re-creates these every render (no correctness impact).
  const setIsinQuery = useCallback((q: string) => setIsinQueryState(q), []);
  const setKeywordQuery = useCallback((q: string) => setKeywordQueryState(q), []);

  const submitSearch = useCallback(
    (mode: LookupMode) => {
      const query = mode === "Isin" ? isinQuery : keywordQuery;
      search.submit(mode, query);
    },
    [isinQuery, keywordQuery, search],
  );

  const selectResult = useCallback((result: AssetLookupResult) => {
    setModalStep({ step: "form-prefilled", selection: result });
  }, []);

  const fillManually = useCallback(() => {
    setModalStep({ step: "form-manual" });
  }, []);

  const back = useCallback(() => {
    setModalStep({ step: "search" });
  }, []);

  const canGoBack = modalStep.step === "form-prefilled";

  return {
    modalStep,
    searchState: search.state,
    isinQuery,
    keywordQuery,
    lastMode: search.lastMode,
    setIsinQuery,
    setKeywordQuery,
    submitSearch,
    retrySearch: search.retry,
    selectResult,
    fillManually,
    back,
    canGoBack,
  };
}
