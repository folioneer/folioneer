import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { Asset } from "@/bindings";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { FormProblem } from "@/ui/components/modal/FormProblem";
import { AssetForm } from "../shared/AssetForm";
import { useEditAssetModal } from "./useEditAssetModal";

interface EditAssetModalProps {
  isOpen: boolean;
  onClose: () => void;
  asset: Asset | null;
  /** When set, the matching form input is focused after the modal opens. */
  focusField?: "reference" | "isin";
}

export function EditAssetModal({ isOpen, onClose, asset, focusField }: EditAssetModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    logger.info("[EditAssetModal] mounted");
  }, []);

  // The 0ms timeout defers .focus() past the modal's mount + zoom-in
  // animation so the input is reachable in the DOM before focus runs.
  useEffect(() => {
    if (!isOpen || !focusField) return;
    const handle = window.setTimeout(() => {
      document.getElementById(`edit-asset-${focusField}`)?.focus();
    }, 0);
    return () => window.clearTimeout(handle);
  }, [isOpen, focusField]);
  const {
    formData,
    error,
    isSubmitting,
    handleChange,
    handleKindChange,
    handleClassChange,
    handleExchangeChange,
    handleSubmit,
    categories,
  } = useEditAssetModal({ asset, onClose });

  const isSubmitDisabled =
    !formData.name.trim() || !formData.reference.trim() || !formData.currency.trim();

  const actions = (
    <>
      <FormProblem idPrefix="edit-asset" error={error ? t(error.key, error.vars) : undefined} />
      <Button id="edit-asset-cancel" variant="secondary" onClick={onClose}>
        {t("action.cancel")}
      </Button>
      <Button
        id="edit-asset-submit"
        type="submit"
        form="edit-asset-form"
        variant="primary"
        loading={isSubmitting}
        disabled={isSubmitDisabled || isSubmitting}
      >
        {t("action.save")}
      </Button>
    </>
  );

  return (
    <Dialog
      id="edit-asset-dialog"
      isOpen={isOpen}
      onClose={onClose}
      title={t("asset.edit_modal_title")}
      actions={actions}
      maxWidth="max-w-xl"
    >
      <form id="edit-asset-form" className="py-2" onSubmit={handleSubmit}>
        <AssetForm
          formData={formData}
          handleChange={handleChange}
          onKindChange={handleKindChange}
          onClassChange={handleClassChange}
          onExchangeChange={handleExchangeChange}
          categories={categories}
          idPrefix="edit-asset"
        />
      </form>
    </Dialog>
  );
}
