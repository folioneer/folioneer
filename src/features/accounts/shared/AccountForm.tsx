import type { ChangeEvent } from "react";
import { useTranslation } from "react-i18next";
import type { UpdateFrequency } from "@/bindings";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextField } from "@/ui/components/field/TextField";
import { FREQUENCY_I18N_KEYS } from "./presenter";

export interface AccountFormData {
  name: string;
  /** ACC-026 — bank or broker brand name; empty string means unset. */
  bank_name: string;
  currency: string;
  update_frequency: UpdateFrequency;
  /** FEE-075 — whether the % management-fee mechanism is enabled on the account. */
  management_fees_enabled: boolean;
}

interface AccountFormProps {
  formData: AccountFormData;
  handleChange: (e: ChangeEvent<HTMLInputElement | HTMLSelectElement>) => void;
  frequencies: UpdateFrequency[];
  idPrefix?: string;
}

export function AccountForm({
  formData,
  handleChange,
  frequencies,
  idPrefix = "account",
}: AccountFormProps) {
  const { t } = useTranslation();

  const frequencyOptions = frequencies.map((freq) => ({
    label: t(FREQUENCY_I18N_KEYS[freq]),
    value: freq,
  }));

  return (
    <div className="space-y-6">
      <TextField
        label={t("account.form_name_label")}
        id={`${idPrefix}-name`}
        name="name"
        required
        placeholder={t("account.form_name_placeholder")}
        value={formData.name}
        onChange={handleChange}
      />

      {/* ACC-026 — optional bank name, free text */}
      <TextField
        label={t("account.form_bank_name_label")}
        id={`${idPrefix}-bank-name`}
        name="bank_name"
        value={formData.bank_name}
        onChange={handleChange}
      />

      <TextField
        label={t("account.form_currency_label")}
        id={`${idPrefix}-currency`}
        name="currency"
        required
        maxLength={3}
        placeholder={t("account.form_currency_placeholder")}
        value={formData.currency}
        onChange={handleChange}
      />

      <SelectField
        label={t("account.form_frequency_label")}
        id={`${idPrefix}-update-frequency`}
        name="update_frequency"
        value={formData.update_frequency}
        onChange={handleChange}
        options={frequencyOptions}
      />

      {/* FEE-075 — opt-in gate for the % management-fee mechanism */}
      <label className="flex items-center gap-3 cursor-pointer group">
        <input
          type="checkbox"
          id={`${idPrefix}-management-fees-enabled`}
          name="management_fees_enabled"
          checked={formData.management_fees_enabled}
          onChange={handleChange}
          className="accent-m3-primary w-4 h-4"
        />
        <span className="text-sm text-m3-on-surface group-hover:text-m3-primary transition-colors">
          {t("account.form_management_fees_label")}
        </span>
      </label>
    </div>
  );
}
