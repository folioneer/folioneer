import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import {
  microToFormatted,
  microToFormattedPrice,
  microToFormattedQuantity,
} from "@/lib/microUnits";
import { Button } from "@/ui/components/button/Button";
import { CalcField } from "@/ui/components/field/CalcField";
import { DateField } from "@/ui/components/field/DateField";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextareaField } from "@/ui/components/field/TextareaField";
import { FormModal } from "@/ui/components/modal/FormModal";
import { formatIsoDateNumeric } from "@/ui/format/date";
import type { TransferDestination, TransferTarget } from "../shared/types";
import { type TransferEditMode, useTransfer } from "./useTransfer";

interface TransferModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** The sending account. */
  accountId: string;
  /** The holding moved (TRF-050). */
  target: TransferTarget;
  /** The portfolio's other accounts (TRF-051). */
  destinations: TransferDestination[];
  onSubmitSuccess: () => void;
  /** Present when correcting a transfer from a journal (TRF-057). */
  editMode?: TransferEditMode;
}

export function TransferModal({
  isOpen,
  onClose,
  accountId,
  target,
  destinations,
  onSubmitSuccess,
  editMode,
}: TransferModalProps) {
  const { t, i18n } = useTranslation();

  useEffect(() => {
    logger.info("[TransferModal] mounted");
  }, []);

  const {
    formData,
    destination,
    betweenCurrencies,
    storedRate,
    refusal,
    isSubmitting,
    isFormValid,
    heldMicro,
    overHolding,
    receivedAboveSent,
    summary,
    handleChange,
    handleSubmit,
  } = useTransfer({ accountId, target, destinations, onSubmitSuccess, editMode });

  // TRF-051 — none chosen by default: an empty first choice until one is picked.
  const accountOptions = useMemo(
    () => [
      ...(formData.toAccountId === "" ? [{ label: "", value: "" }] : []),
      ...destinations.map((d) => ({
        label: t("transfer.account_option", { name: d.name, currency: d.currency }),
        value: d.id,
      })),
    ],
    [destinations, formData.toAccountId, t],
  );

  const held = microToFormattedQuantity(heldMicro);
  // TRF-054 — over the quantity held, refused under the field; TRF-055 — a refusal for lack
  // of units shows there too.
  const sentError = overHolding
    ? t("transfer.error_over_holding", { held, reference: target.assetReference })
    : refusal?.onQuantity
      ? t(refusal.message.key, refusal.message.vars)
      : undefined;
  const receivedError = receivedAboveSent ? t("transfer.error_received_above_sent") : undefined;

  const rateDate = formatIsoDateNumeric(formData.date, i18n.language);
  const cost =
    summary === null || destination === null
      ? ""
      : summary.convertedMicro !== null && summary.rateMicro !== null
        ? t("transfer.summary_cost_converted", {
            cost: microToFormatted(summary.costMicro, 2),
            from: target.accountCurrency,
            converted: microToFormatted(summary.convertedMicro, 2),
            to: destination.currency,
            rate: microToFormattedPrice(summary.rateMicro),
          })
        : t("transfer.summary_cost", {
            cost: microToFormatted(summary.costMicro, 2),
            currency: target.accountCurrency,
          });

  const footer = (
    <div className="flex items-center justify-end gap-2">
      <Button id="transfer-cancel" variant="secondary" onClick={onClose} disabled={isSubmitting}>
        {t("action.cancel")}
      </Button>
      <Button
        type="submit"
        form="transfer-form"
        id="transfer-submit"
        variant="primary"
        loading={isSubmitting}
        disabled={isSubmitting || !isFormValid || refusal !== null}
      >
        {t("transfer.action_record")}
      </Button>
    </div>
  );

  return (
    <FormModal
      id="transfer-modal"
      isOpen={isOpen}
      onClose={onClose}
      title={t("transfer.modal_title")}
      footer={footer}
      maxWidth="max-w-2xl"
    >
      <form id="transfer-form" onSubmit={handleSubmit} className="flex flex-col gap-4">
        {/* TRF-051 — the asset, what is held, its average price and the sending account */}
        <p id="transfer-holding" className="text-sm text-m3-on-surface">
          {t("write_off.asset_label")} <span className="font-semibold">{target.assetName}</span>
          {" · "}
          {t("transfer.holding_line", {
            quantity: held,
            reference: target.assetReference,
            price: microToFormattedPrice(target.averagePriceMicro),
            account: target.accountName,
          })}
        </p>

        <DateField
          id="transfer-date"
          label={t("transaction.form_date_label")}
          value={formData.date}
          onChange={(e) => handleChange("date", e.target.value)}
          required
        />

        <SelectField
          id="transfer-to-account"
          label={t("transfer.form_to_label")}
          value={formData.toAccountId}
          onChange={(e) => handleChange("toAccountId", e.target.value)}
          options={accountOptions}
          disabled={!!editMode}
          required
        />

        <div className="grid grid-cols-2 gap-4">
          <CalcField
            id="transfer-sent"
            label={t("transfer.form_sent_label")}
            value={formData.sent}
            onValueChange={(v) => handleChange("sent", v)}
            error={sentError}
            required
          />
          <CalcField
            id="transfer-received"
            label={t("transfer.form_received_label")}
            value={formData.received}
            onValueChange={(v) => handleChange("received", v)}
            error={receivedError}
            required
          />
        </div>

        {/* TRF-052 — between currencies, the rate the core would take, typed over at will */}
        {betweenCurrencies && destination && (
          <div className="flex flex-col gap-1">
            <CalcField
              id="transfer-rate"
              label={t("transfer.form_rate_label", {
                from: target.accountCurrency,
                to: destination.currency,
              })}
              value={formData.rate}
              onValueChange={(v) => handleChange("rate", v)}
              required
            />
            {(storedRate === "stored" || storedRate === "none") && (
              <p id="transfer-rate-hint" className="text-xs text-m3-on-surface-variant">
                {t(storedRate === "stored" ? "transfer.rate_stored" : "transfer.rate_none", {
                  date: rateDate,
                })}
              </p>
            )}
          </div>
        )}

        {/* TRF-053 — what moves, at the average price shown */}
        {summary !== null && destination !== null && refusal === null && (
          <div
            id="transfer-summary"
            className="flex flex-col gap-1 rounded-xl bg-m3-surface-container-high px-4 py-3 text-sm"
          >
            <p className="text-m3-on-surface">
              {t("transfer.summary", {
                received: formData.received,
                reference: target.assetReference,
                cost,
                account: destination.name,
              })}
            </p>
            {summary.feeMicro > 0 && (
              <p id="transfer-summary-fee" className="text-m3-on-surface-variant">
                {t("transfer.summary_fee", {
                  fee: microToFormattedQuantity(summary.feeMicro),
                  reference: target.assetReference,
                  cost: microToFormatted(summary.feeCostMicro, 2),
                  currency: target.accountCurrency,
                })}
              </p>
            )}
          </div>
        )}

        <TextareaField
          id="transfer-note"
          label={t("transaction.form_note_label")}
          rows={2}
          value={formData.note}
          onChange={(e) => handleChange("note", e.target.value)}
          placeholder={t("transaction.form_note_placeholder")}
        />

        {refusal && !refusal.onQuantity && (
          <p id="transfer-error" role="alert" className="text-sm text-m3-error">
            {t(refusal.message.key, refusal.message.vars)}
          </p>
        )}
      </form>
    </FormModal>
  );
}
