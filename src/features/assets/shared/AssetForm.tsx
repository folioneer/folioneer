import { useTranslation } from "react-i18next";
import type { AssetCategory, AssetClass, Exchange } from "@/bindings";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextField } from "@/ui/components/field/TextField";
import { ASSET_CLASSES, RISK_LEVELS } from "./constants";
import { ExchangePicker } from "./ExchangePicker";

interface AssetFormData {
  name: string;
  reference: string;
  isin: string;
  class: AssetClass;
  currency: string;
  risk_level: number;
  category_id: string;
  exchange: Exchange | null;
  /** AST-024 — whether the asset is an eligible Interest-credit target. */
  interest_bearing: boolean;
}

interface AssetFormProps {
  formData: AssetFormData;
  handleChange: (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => void;
  onClassChange?: (assetClass: AssetClass) => void;
  onExchangeChange: (exchange: Exchange | null) => void;
  categories: AssetCategory[];
  duplicateWarning?: boolean;
  idPrefix?: string;
}

export function AssetForm({
  formData,
  handleChange,
  onClassChange,
  onExchangeChange,
  categories,
  duplicateWarning = false,
  idPrefix = "asset",
}: AssetFormProps) {
  const { t } = useTranslation();

  const categoryOptions = categories.map((cat) => ({
    label: cat.name,
    value: cat.id,
  }));

  const classOptions = ASSET_CLASSES.map((c) => ({
    label: c,
    value: c,
  }));

  const handleClassSelect = (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
    handleChange(e);
    if (onClassChange) {
      onClassChange(e.target.value as AssetClass);
    }
  };

  return (
    <div className="space-y-6">
      <TextField
        label={t("asset.form_name_label")}
        id={`${idPrefix}-name`}
        name="name"
        required
        placeholder={t("asset.form_name_placeholder")}
        value={formData.name}
        onChange={handleChange}
      />

      <div className="grid grid-cols-2 gap-4">
        <div className="flex flex-col gap-1.5">
          <TextField
            label={t("asset.form_reference_label")}
            id={`${idPrefix}-reference`}
            name="reference"
            required
            placeholder={t("asset.form_reference_placeholder")}
            value={formData.reference}
            onChange={handleChange}
            aria-describedby={duplicateWarning ? `${idPrefix}-reference-warning` : undefined}
          />
          {duplicateWarning && (
            <p
              id={`${idPrefix}-reference-warning`}
              role="alert"
              className="text-xs text-m3-tertiary bg-m3-tertiary-container/40 rounded-lg px-3 py-2"
            >
              {t("asset.warning_duplicate_reference")}
            </p>
          )}
        </div>
        <TextField
          label={t("asset.form_currency_label")}
          id={`${idPrefix}-currency`}
          name="currency"
          required
          className="uppercase"
          placeholder={t("asset.form_currency_placeholder")}
          value={formData.currency}
          onChange={handleChange}
        />
      </div>

      <TextField
        label={t("asset.form_isin_label")}
        id={`${idPrefix}-isin`}
        name="isin"
        className="uppercase"
        placeholder={t("asset.form_isin_placeholder")}
        value={formData.isin}
        onChange={handleChange}
      />

      <SelectField
        label={t("asset.form_category_label")}
        id={`${idPrefix}-category`}
        name="category_id"
        value={formData.category_id}
        onChange={handleChange}
        options={categoryOptions}
      />

      <SelectField
        label={t("asset.form_class_label")}
        id={`${idPrefix}-class`}
        name="class"
        value={formData.class}
        onChange={handleClassSelect}
        options={classOptions}
      />

      <ExchangePicker value={formData.exchange} onChange={onExchangeChange} idPrefix={idPrefix} />

      {/* AST-024 — Interest-credit eligibility flag */}
      <label className="flex items-center gap-3 cursor-pointer group">
        <input
          type="checkbox"
          id={`${idPrefix}-interest-bearing`}
          name="interest_bearing"
          checked={formData.interest_bearing}
          onChange={handleChange}
          className="accent-m3-primary w-4 h-4"
        />
        <span className="text-sm text-m3-on-surface group-hover:text-m3-primary transition-colors">
          {t("asset.form_interest_bearing_label")}
        </span>
      </label>

      <fieldset className="flex flex-col gap-1.5 border-none p-0 m-0">
        <legend className="m3-input-label">{t("asset.form_risk_label")}</legend>
        <div
          role="radiogroup"
          aria-labelledby={`${idPrefix}-risk-label`}
          className="flex p-1 bg-m3-surface-variant rounded-2xl gap-1 overflow-hidden"
        >
          {RISK_LEVELS.map((level) => {
            const isSelected = formData.risk_level === level;

            return (
              <label
                key={level}
                className={`
                  relative flex-1 flex items-center justify-center py-2 rounded-xl
                  text-sm font-bold cursor-pointer transition-all duration-200
                  focus-within:ring-2 focus-within:ring-m3-primary focus-within:ring-offset-1
                  ${
                    isSelected
                      ? "bg-m3-primary text-m3-on-primary"
                      : "text-m3-on-surface-variant hover:bg-m3-primary/10"
                  }
                `}
              >
                <input
                  type="radio"
                  name="risk_level"
                  value={level}
                  checked={isSelected}
                  onChange={handleChange}
                  className="absolute opacity-0 w-0 h-0 appearance-none"
                />
                <span>{level}</span>
              </label>
            );
          })}
        </div>
      </fieldset>
    </div>
  );
}
