import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { AssetKind, AssetLookupResult } from "@/bindings";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { FormProblem } from "@/ui/components/modal/FormProblem";
import { AssetForm } from "../shared/AssetForm";
import { useAddAsset } from "./useAddAsset";

interface AddAssetModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** AST-040 — the kind chosen; changing it is the caller's, which may show the lookup instead. */
  kind: AssetKind;
  onKindChange: (kind: AssetKind) => void;
  prefill?: AssetLookupResult;
  onBack?: () => void;
  onSuccess?: (assetId: string) => void;
}

export function AddAssetModal({
  isOpen,
  onClose,
  kind,
  onKindChange,
  prefill,
  onBack,
  onSuccess,
}: AddAssetModalProps) {
  const { t } = useTranslation();
  const {
    formData,
    referenceProposed,
    error,
    isSubmitting,
    handleChange,
    handleClassChange,
    handleExchangeChange,
    handleSubmit,
    categories,
  } = useAddAsset({
    kind,
    prefill,
    onSubmitSuccess: (assetId) => {
      onSuccess?.(assetId);
      onClose();
    },
  });

  useEffect(() => {
    logger.info("[AddAssetModal] mounted");
  }, []);

  const isSubmitDisabled =
    !formData.name.trim() || !formData.reference.trim() || !formData.currency.trim();

  // The refusal sits on its own line above the actions: beside "Back to the search" it
  // would leave neither room to be read.
  const actions = (
    <div className="flex w-full flex-col gap-3">
      <FormProblem idPrefix="add-asset" error={error ? t(error.key, error.vars) : undefined} />
      <div className="flex items-center justify-end gap-2">
        {onBack && (
          <Button
            id="add-asset-back"
            variant="outline"
            onClick={onBack}
            className="mr-auto whitespace-nowrap"
          >
            {t("asset.web_lookup.action_back")}
          </Button>
        )}
        <Button id="add-asset-cancel" variant="secondary" onClick={onClose}>
          {t("action.cancel")}
        </Button>
        <Button
          id="add-asset-submit"
          type="submit"
          form="add-asset-form"
          variant="primary"
          loading={isSubmitting}
          disabled={isSubmitDisabled || isSubmitting}
        >
          {t("action.add")}
        </Button>
      </div>
    </div>
  );

  return (
    <Dialog
      id="add-asset-dialog"
      isOpen={isOpen}
      onClose={onClose}
      title={t("asset.add_modal_title")}
      actions={actions}
      maxWidth="max-w-xl"
    >
      <form id="add-asset-form" className="py-2" onSubmit={handleSubmit}>
        <AssetForm
          formData={formData}
          handleChange={handleChange}
          onKindChange={onKindChange}
          onClassChange={handleClassChange}
          onExchangeChange={handleExchangeChange}
          categories={categories}
          referenceProposed={referenceProposed}
          idPrefix="add-asset"
        />
      </form>
    </Dialog>
  );
}
