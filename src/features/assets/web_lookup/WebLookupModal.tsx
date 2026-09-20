import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { AssetLookupResult } from "@/bindings";
import { logger } from "@/lib/logger";
import { Dialog } from "@/ui/components/modal/Dialog";
import { AddAssetModal } from "../add_asset/AddAsset";
import { SearchPanel } from "./SearchPanel";
import { useWebLookupModal } from "./useWebLookupModal";

interface WebLookupModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** When set (from URL `createNew` param), skips search and opens form directly. */
  prefillName?: string;
  onSuccess?: (assetId: string) => void;
}

export function WebLookupModal({ isOpen, onClose, prefillName, onSuccess }: WebLookupModalProps) {
  const { t } = useTranslation();
  const {
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

  // URL-originated shortcut: skip search and open form directly (WEB-010)
  if (namePrefill && modalStep.step === "search") {
    return (
      <AddAssetModal
        isOpen={isOpen}
        onClose={onClose}
        prefill={namePrefill}
        onSuccess={onSuccess}
      />
    );
  }

  if (modalStep.step === "form-prefilled") {
    return (
      <AddAssetModal
        isOpen={isOpen}
        onClose={onClose}
        prefill={modalStep.selection}
        onBack={canGoBack ? back : undefined}
        onSuccess={onSuccess}
      />
    );
  }

  if (modalStep.step === "form-manual") {
    return <AddAssetModal isOpen={isOpen} onClose={onClose} onSuccess={onSuccess} />;
  }

  return (
    <Dialog
      id="web-lookup-dialog"
      isOpen={isOpen}
      onClose={onClose}
      title={t("asset.web_lookup.title")}
      maxWidth="max-w-xl"
    >
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
    </Dialog>
  );
}
