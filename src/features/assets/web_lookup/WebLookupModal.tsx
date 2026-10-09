import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { AssetLookupResult } from "@/bindings";
import { logger } from "@/lib/logger";
import { Dialog } from "@/ui/components/modal/Dialog";
import { AddAssetModal } from "../add_asset/AddAsset";
import { KindPicker } from "../shared/KindPicker";
import { SearchPanel } from "./SearchPanel";
import { useWebLookupModal } from "./useWebLookupModal";

interface WebLookupModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** When set (from URL `createNew` param), skips search and opens form directly. */
  prefillName?: string;
  onSuccess?: (assetId: string) => void;
}

/**
 * AST-040 — the "New asset" dialog: the kind first, then the lookup for a kind that starts
 * from it (WEB-010), or that kind's form.
 */
export function WebLookupModal({ isOpen, onClose, prefillName, onSuccess }: WebLookupModalProps) {
  const { t } = useTranslation();
  const {
    kind,
    selectKind,
    modalStep,
    searchState,
    isinQuery,
    keywordQuery,
    lastMode,
    setIsinQuery,
    setKeywordQuery,
    submitSearch,
    retrySearch,
    selectResult,
    fillManually,
    back,
    canGoBack,
  } = useWebLookupModal();

  useEffect(() => {
    logger.info("[WebLookupModal] mounted");
  }, []);

  // Stable identity — only recreated when prefillName changes (WEB-010 URL shortcut)
  const namePrefill = useMemo<AssetLookupResult | undefined>(
    () =>
      prefillName
        ? {
            name: prefillName,
            reference: null,
            isin: null,
            currency: null,
            asset_class: null,
            exchange: null,
          }
        : undefined,
    [prefillName],
  );

  if (!isOpen) return null;

  // URL-originated shortcut: a name is already known, so the form opens directly (WEB-010)
  if (modalStep.step !== "search" || namePrefill) {
    return (
      <AddAssetModal
        isOpen={isOpen}
        onClose={onClose}
        kind={kind}
        onKindChange={selectKind}
        prefill={modalStep.step === "form-prefilled" ? modalStep.selection : namePrefill}
        onBack={canGoBack ? back : undefined}
        onSuccess={onSuccess}
      />
    );
  }

  return (
    <Dialog
      id="web-lookup-dialog"
      isOpen={isOpen}
      onClose={onClose}
      title={t("asset.add_modal_title")}
      maxWidth="max-w-xl"
    >
      <div className="flex flex-col gap-6 py-2">
        <KindPicker value={kind} onChange={selectKind} idPrefix="add-asset" />
        <SearchPanel
          isinQuery={isinQuery}
          keywordQuery={keywordQuery}
          state={searchState}
          lastMode={lastMode}
          submit={submitSearch}
          retry={retrySearch}
          setIsinQuery={setIsinQuery}
          setKeywordQuery={setKeywordQuery}
          onSelect={selectResult}
          onFillManually={fillManually}
        />
      </div>
    </Dialog>
  );
}
