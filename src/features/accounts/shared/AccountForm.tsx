import type { ChangeEvent } from "react";
import { useTranslation } from "react-i18next";
import type { CostBasisMethod, ImpliedCostBasis, ProfileSummary, WrapperLists } from "@/bindings";
import { ComboboxField } from "@/ui/components/field/ComboboxField";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextField } from "@/ui/components/field/TextField";
import type { I18nMessage } from "@/ui/format/i18n";
import { countryOptions, impliedCostBasisHint, profileOptions, wrapperOptions } from "./presenter";

export interface AccountFormData {
  name: string;
  /** ACC-026 — bank or broker brand name; empty string means unset. */
  bank_name: string;
  currency: string;
  /** FEE-075 — whether the % management-fee mechanism is enabled on the account. */
  management_fees_enabled: boolean;
  /** ACC-041 — the account's own country; "" takes the portfolio's. */
  country: string;
  /** ACC-043 — the identifier of its tax wrapper; "" for none. */
  wrapper: string;
  /** CBM-011A — the cost basis method chosen; "" follows the country's (CBM-014). */
  cost_basis_method: CostBasisMethod | "";
  /** PFL-021 / PFL-031 — the profile it belongs to; "" until the profiles are read. */
  profile_id: string;
}

interface AccountFormProps {
  formData: AccountFormData;
  handleChange: (e: ChangeEvent<HTMLInputElement | HTMLSelectElement>) => void;
  /** ACC-045 — the country chosen in the searchable list ("" for the portfolio's). */
  setCountry: (country: string) => void;
  /** ACC-045 — the wrapper lists; the two fields show once they are read. */
  wrapperLists: WrapperLists | null;
  /** CBM-014 — the method the country implies, once read. */
  impliedCostBasis?: ImpliedCostBasis | null;
  /** #515 (the validated mock) — an edited account's method changed: its gains recompute. */
  costBasisChanged?: boolean;
  /** PFL-021 / PFL-031 — the profiles to choose from; the field shows once they are read. */
  profiles?: ProfileSummary[] | null;
  /** #767 (the validated mock) — an edited account changes profile: it moves with its history. */
  profileMoveWarning?: I18nMessage | null;
  idPrefix?: string;
}

export function AccountForm({
  formData,
  handleChange,
  setCountry,
  wrapperLists,
  impliedCostBasis = null,
  costBasisChanged = false,
  profiles = null,
  profileMoveWarning = null,
  idPrefix = "account",
}: AccountFormProps) {
  const { t, i18n } = useTranslation();
  const hint =
    formData.cost_basis_method === "" && impliedCostBasis
      ? impliedCostBasisHint(impliedCostBasis, i18n.language)
      : null;

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

      {/* PFL-021 / PFL-031 (#767, the validated mock) — the profile the account belongs to */}
      {profiles && (
        <div className="flex flex-col gap-1.5">
          <SelectField
            label={t("account.form_profile_label")}
            id={`${idPrefix}-profile`}
            name="profile_id"
            value={formData.profile_id}
            onChange={handleChange}
            options={profileOptions(profiles, t("profile.default_name"))}
          />
          {profileMoveWarning ? (
            <span
              id={`${idPrefix}-profile-warning`}
              role="status"
              className="text-xs text-m3-error"
            >
              {t(profileMoveWarning.key, profileMoveWarning.vars)}
            </span>
          ) : (
            <span className="text-xs text-m3-on-surface-variant">
              {t("account.form_profile_hint")}
            </span>
          )}
        </div>
      )}

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

      {/* ACC-045, ACC-048 — the account's country, then its wrapper */}
      {wrapperLists && (
        <>
          <ComboboxField
            label={t("account.form_country_label")}
            id={`${idPrefix}-country`}
            items={countryOptions(wrapperLists, i18n.language).map((option) => ({
              id: option.value,
              label:
                option.value === ""
                  ? t("account.form_country_portfolio", { country: option.country })
                  : option.label,
            }))}
            displayKey="label"
            idKey="id"
            value={formData.country}
            onChange={setCountry}
          />
          <div className="flex flex-col gap-1.5">
            <SelectField
              label={t("account.form_wrapper_label")}
              id={`${idPrefix}-wrapper`}
              name="wrapper"
              value={formData.wrapper}
              onChange={handleChange}
              options={[
                { value: "", label: t("account.form_wrapper_none") },
                ...wrapperOptions(wrapperLists, formData.country, formData.wrapper, i18n.language),
              ]}
            />
            <span className="text-xs text-m3-on-surface-variant">
              {t("account.form_wrapper_hint")}
            </span>
          </div>
        </>
      )}

      {/* #515 (the validated mock) — the cost basis method, preselected from the country */}
      <div className="flex flex-col gap-1.5">
        <SelectField
          label={t("account.form_cost_basis_label")}
          id={`${idPrefix}-cost-basis-method`}
          name="cost_basis_method"
          value={formData.cost_basis_method || impliedCostBasis?.method || "WeightedAverage"}
          onChange={handleChange}
          options={[
            { value: "Fifo", label: t("account.form_cost_basis_fifo") },
            { value: "WeightedAverage", label: t("account.form_cost_basis_weighted_average") },
          ]}
        />
        {costBasisChanged ? (
          <span id={`${idPrefix}-cost-basis-warning`} className="text-xs text-m3-error">
            {t("account.form_cost_basis_change_warning")}
          </span>
        ) : (
          hint && (
            <span className="text-xs text-m3-on-surface-variant">{t(hint.key, hint.vars)}</span>
          )
        )}
      </div>

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
