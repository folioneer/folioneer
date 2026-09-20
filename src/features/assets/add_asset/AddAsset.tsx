import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { AssetLookupResult } from "@/bindings";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { AssetForm } from "../shared/AssetForm";
import { useAddAsset } from "./useAddAsset";

interface AddAssetModalProps {
  isOpen: boolean;
  onClose: () => void;
  prefill?: AssetLookupResult;
  onBack?: () => void;
  onSuccess?: (assetId: string) => void;
}

export function AddAssetModal({ isOpen, onClose, prefill, onBack, onSuccess }: AddAssetModalProps) {
  const { t } = useTranslation();
  const {
    formData,
    error,
    isSubmitting,
    duplicateWarning,
    handleChange,
    handleClassChange,
    handleExchangeChange,
    handleSubmit,
    categories,
  } = useAddAsset({
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

  const actions = (
    <>
      {onBack && (
        <Button
          variant="ghost"
          aria-label={t("asset.web_lookup.action_back")}
          onClick={onBack}
          className="mr-auto"
        >
          {t("asset.web_lookup.action_back")}
        </Button>
      )}
      <Button variant="secondary" onClick={onClose}>
        {t("action.cancel")}
      </Button>
      <Button
        type="submit"
        form="add-asset-form"
        variant="primary"
        loading={isSubmitting}
        disabled={isSubmitDisabled || isSubmitting}
      >
        {t("action.add")}
      </Button>
    </>
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
          onClassChange={handleClassChange}
          onExchangeChange={handleExchangeChange}
          categories={categories}
          duplicateWarning={duplicateWarning}
          idPrefix="add-asset"
        />
        {error && (
          <p role="alert" className="mt-3 text-sm text-m3-error">
            {t(error.key, error.vars)}
          </p>
        )}
      </form>
    </Dialog>
  );
}
