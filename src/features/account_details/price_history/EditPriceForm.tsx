import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { AssetPrice } from "@/bindings";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { DateField } from "@/ui/components/field/DateField";
import { TextField } from "@/ui/components/field/TextField";
import { FormModal } from "@/ui/components/modal/FormModal";
import { useEditPrice } from "./useEditPrice";

interface EditPriceFormProps {
  isOpen: boolean;
  assetId: string;
  assetName: string;
  assetCurrency: string;
  target: AssetPrice;
  onSuccess: () => void;
  onBack: () => void;
  onClose: () => void;
}

export function EditPriceForm({
  isOpen,
  assetName,
  assetCurrency,
  target,
  assetId,
  onSuccess,
  onBack,
  onClose,
}: EditPriceFormProps) {
  const { t } = useTranslation();
  const { date, price, setDate, setPrice, isValid, isSubmitting, error, handleSubmit } =
    useEditPrice({ assetId, target, onSuccess });

  useEffect(() => {
    logger.info("[EditPriceForm] mounted", { assetId, date: target.date });
  }, [assetId, target.date]);

  return (
    <FormModal
      id="edit-price-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t("price_history.edit_title")}
      maxWidth="max-w-md"
      footer={
        <div className="flex items-center justify-between gap-2">
          <Button variant="ghost" onClick={onBack} disabled={isSubmitting}>
            {t("action.back")}
          </Button>
          <div className="flex gap-2">
            <Button variant="secondary" onClick={onClose} disabled={isSubmitting}>
              {t("action.cancel")}
            </Button>
            <Button
              type="submit"
              form="edit-price-form"
              variant="primary"
              loading={isSubmitting}
              disabled={isSubmitting || !isValid}
            >
              {t("price_history.save")}
            </Button>
          </div>
        </div>
      }
    >
      <form
        id="edit-price-form"
        onSubmit={(e) => {
          e.preventDefault();
          handleSubmit();
        }}
        className="flex flex-col gap-4"
      >
        <TextField
          id="edit-price-asset"
          label={t("price_modal.asset_label")}
          type="text"
          value={assetName}
          readOnly
          aria-readonly="true"
        />
        <DateField
          id="edit-price-date"
          label={t("price_modal.date_label")}
          value={date}
          onChange={(e) => setDate(e.target.value)}
          required
        />
        <CalcField
          id="edit-price-price"
          label={`${t("price_modal.price_label")} (${assetCurrency})`}
          value={price}
          onValueChange={(v) => setPrice(v)}
          placeholder={t("price_modal.form_price_placeholder")}
          required
        />
        {error && (
          <p role="alert" className="text-sm text-m3-error">
            {t(`error.${error}`, { defaultValue: error })}
          </p>
        )}
      </form>
    </FormModal>
  );
}
