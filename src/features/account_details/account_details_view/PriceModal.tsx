import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { ComboboxField } from "@/ui/components/field/ComboboxField";
import { DateField } from "@/ui/components/field/DateField";
import { FormModal } from "@/ui/components/modal/FormModal";
import type { PriceableAsset } from "../shared/types";
import { usePriceModal } from "./usePriceModal";

interface PriceModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** Active non-cash holdings selectable in the asset combobox (MKT-011). */
  assets: PriceableAsset[];
  /** Asset pre-selected when the modal opens. */
  initialAssetId: string;
  /** Account whose stored last-operation date seeds the date field. */
  accountId: string;
  onSubmitSuccess: () => void;
  /** Refresh-only callback for "record & add another" — keeps the modal open (MKT-014). */
  onRecorded: (assetId: string) => void;
}

export function PriceModal({
  isOpen,
  onClose,
  assets,
  initialAssetId,
  accountId,
  onSubmitSuccess,
  onRecorded,
}: PriceModalProps) {
  const { t } = useTranslation();
  const {
    assetId,
    date,
    price,
    selectedCurrency,
    error,
    isSubmitting,
    isFormValid,
    handleAssetChange,
    handleChange,
    handleSubmit,
    handleAddAnother,
  } = usePriceModal({ assets, initialAssetId, accountId, onSubmitSuccess, onRecorded });

  useEffect(() => {
    logger.info("[PriceModal] mounted");
  }, []);

  const footer = useMemo(
    () => (
      <div className="flex items-center justify-end gap-2">
        <Button
          id="price-modal-cancel"
          variant="secondary"
          onClick={onClose}
          disabled={isSubmitting}
        >
          {t("action.cancel")}
        </Button>
        <Button
          id="price-modal-add-another"
          type="button"
          variant="secondary"
          onClick={handleAddAnother}
          loading={isSubmitting}
          disabled={isSubmitting || !isFormValid}
        >
          {t("price_modal.submit_and_add_another")}
        </Button>
        <Button
          id="price-modal-submit"
          type="submit"
          form="price-modal-form"
          variant="primary"
          loading={isSubmitting}
          disabled={isSubmitting || !isFormValid}
        >
          {t("price_modal.submit")}
        </Button>
      </div>
    ),
    [isSubmitting, isFormValid, t, onClose, handleAddAnother],
  );

  return (
    <FormModal
      id="price-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t("price_modal.title")}
      footer={footer}
      maxWidth="max-w-md"
    >
      <form id="price-modal-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
        {/* Asset — fuzzy-search combobox, pre-selected to the launched holding (MKT-011) */}
        <ComboboxField
          id="price-modal-asset"
          label={t("price_modal.asset_label")}
          items={assets}
          displayKey="assetName"
          idKey="assetId"
          value={assetId}
          onChange={handleAssetChange}
          searchKeys={["assetName"]}
          placeholder={t("price_modal.asset_placeholder")}
        />

        {/* Date — editable, pre-filled with the account's stored last-operation date (MKT-011) */}
        <DateField
          id="price-modal-date"
          label={t("price_modal.date_label")}
          value={date}
          onChange={(e) => handleChange("date", e.target.value)}
          required
        />

        {/* Price with currency label (MKT-023) */}
        <CalcField
          id="price-modal-price"
          label={`${t("price_modal.price_label")} (${selectedCurrency})`}
          value={price}
          onValueChange={(v) => handleChange("price", v)}
          placeholder={t("price_modal.form_price_placeholder")}
          required
        />

        {/* Inline error (MKT-029) */}
        {error && (
          <p role="alert" className="text-sm text-m3-error">
            {t(error.key, error.vars)}
          </p>
        )}
      </form>
    </FormModal>
  );
}
