import { useCallback, useState } from "react";
import type { AssetKind, AssetLookupResult, LookupMode } from "@/bindings";
import { DEFAULT_ASSET_KIND, kindFormOf, kindOffering } from "../shared/creationDefaults";
import type { WebLookupSearchState } from "./useWebLookupSearch";
import { useWebLookupSearch } from "./useWebLookupSearch";

export type ModalStep =
  | { step: "search" }
  | { step: "form-prefilled"; selection: AssetLookupResult }
  | { step: "form-manual" };

export interface UseWebLookupModalReturn {
  /** AST-040 — the kind chosen; a kind whose form starts from the lookup opens on the search. */
  kind: AssetKind;
  selectKind: (kind: AssetKind) => void;
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

/** The step a kind's form starts on: the search when the core says it starts from the lookup (AST-037). */
function firstStepOf(kind: AssetKind): ModalStep {
  return kindFormOf(kind)?.has_lookup ? { step: "search" } : { step: "form-manual" };
}

export function useWebLookupModal(): UseWebLookupModalReturn {
  const search = useWebLookupSearch();
  const [isinQuery, setIsinQueryState] = useState("");
  const [keywordQuery, setKeywordQueryState] = useState("");
  const [kind, setKind] = useState<AssetKind>(DEFAULT_ASSET_KIND);
  const [modalStep, setModalStep] = useState<ModalStep>(() => firstStepOf(DEFAULT_ASSET_KIND));

  // reviewer-frontend FP: `[search]` re-creates these every render (no correctness impact).
  const setIsinQuery = useCallback((q: string) => setIsinQueryState(q), []);
  const setKeywordQuery = useCallback((q: string) => setKeywordQueryState(q), []);

  const selectKind = useCallback((next: AssetKind) => {
    setKind(next);
    setModalStep(firstStepOf(next));
  }, []);

  const submitSearch = useCallback(
    (mode: LookupMode) => {
      const query = mode === "Isin" ? isinQuery : keywordQuery;
      search.submit(mode, query);
    },
    [isinQuery, keywordQuery, search],
  );

  // WEB-041 — a result of a class the kind searched does not offer (a crypto asset found
  // from the listed kind) opens on the kind that offers it.
  const selectResult = useCallback((result: AssetLookupResult) => {
    setKind((current) => kindOffering(current, result.asset_class));
    setModalStep({ step: "form-prefilled", selection: result });
  }, []);

  const fillManually = useCallback(() => {
    setModalStep({ step: "form-manual" });
  }, []);

  const back = useCallback(() => {
    setKind((current) => (kindFormOf(current)?.has_lookup ? current : DEFAULT_ASSET_KIND));
    setModalStep({ step: "search" });
  }, []);

  const canGoBack = modalStep.step === "form-prefilled";

  return {
    kind,
    selectKind,
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
