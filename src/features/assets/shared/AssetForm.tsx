import { useTranslation } from "react-i18next";
import type { AssetCategory, AssetClass, AssetKind, Exchange } from "@/bindings";
import { SelectField } from "@/ui/components/field/SelectField";
import { TextField } from "@/ui/components/field/TextField";
import { kindFormOf, RISK_LEVELS } from "./creationDefaults";
import { ExchangePicker } from "./ExchangePicker";
import { KindPicker } from "./KindPicker";
import { currencyLabelKey, formatAssetClass, referenceLabelKey } from "./presenter";

interface AssetFormData {
  kind: AssetKind;
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
  onKindChange: (kind: AssetKind) => void;
  onClassChange?: (assetClass: AssetClass) => void;
  onExchangeChange: (exchange: Exchange | null) => void;
  categories: AssetCategory[];
  /** AST-038 — the reference shown was proposed from the name and not yet typed over. */
  referenceProposed?: boolean;
  idPrefix?: string;
}

/**
 * AST-040 — the fields of an asset, for the kind chosen: which ones show, and the classes
 * offered, are read from the core's description of that kind's form (AST-037).
 */
export function AssetForm({
  formData,
  handleChange,
  onKindChange,
  onClassChange,
  onExchangeChange,
  categories,
  referenceProposed = false,
  idPrefix = "asset",
}: AssetFormProps) {
  const { t } = useTranslation();
  const kindForm = kindFormOf(formData.kind);

  const categoryOptions = categories.map((cat) => ({
    label: cat.name,
    value: cat.id,
  }));

  const classOptions = (kindForm?.classes ?? []).map((entry) => ({
    label: formatAssetClass(entry.class, t),
    value: entry.class,
  }));

  const handleClassSelect = (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
    handleChange(e);
    if (onClassChange) {
      onClassChange(e.target.value as AssetClass);
    }
  };

  const nameField = (
    <TextField
      label={t("asset.form_name_label")}
      id={`${idPrefix}-name`}
      name="name"
      required
      placeholder={t("asset.form_name_placeholder")}
      value={formData.name}
      onChange={handleChange}
    />
  );

  const referenceField = (
    <div className="flex flex-col gap-1">
      <TextField
        label={t(referenceLabelKey(formData.kind))}
        id={`${idPrefix}-reference`}
        name="reference"
        required
        className="uppercase"
        value={formData.reference}
        onChange={handleChange}
        aria-describedby={referenceProposed ? `${idPrefix}-reference-proposed` : undefined}
      />
      {referenceProposed && (
        <p
          id={`${idPrefix}-reference-proposed`}
          className="text-xs text-m3-on-surface-variant ml-1"
        >
          {t("asset.form_reference_proposed")}
        </p>
      )}
    </div>
  );

  const currencyField = (
    <TextField
      label={t(currencyLabelKey(formData.kind))}
      id={`${idPrefix}-currency`}
      name="currency"
      required
      className="uppercase"
      placeholder={t("asset.form_currency_placeholder")}
      value={formData.currency}
      onChange={handleChange}
    />
  );

  const categoryField = (
    <SelectField
      label={t("asset.form_category_label")}
      id={`${idPrefix}-category`}
      name="category_id"
      value={formData.category_id}
      onChange={handleChange}
      options={categoryOptions}
    />
  );

  return (
    <div className="flex flex-col gap-6">
      <KindPicker value={formData.kind} onChange={onKindChange} idPrefix={idPrefix} />

      {kindForm?.has_isin ? (
        <>
          {nameField}
          <div className="grid grid-cols-2 gap-4">
            <TextField
              label={t("asset.form_isin_label")}
              id={`${idPrefix}-isin`}
              name="isin"
              required
              className="uppercase"
              placeholder={t("asset.form_isin_placeholder")}
              value={formData.isin}
              onChange={handleChange}
            />
            {currencyField}
          </div>
          <div className="grid grid-cols-3 gap-4">
            <div className="col-span-2">
              <ExchangePicker
                value={formData.exchange}
                onChange={onExchangeChange}
                idPrefix={idPrefix}
              />
            </div>
            {referenceField}
          </div>
        </>
      ) : kindForm?.proposes_reference ? (
        <>
          {nameField}
          <div className="grid grid-cols-2 gap-4">
            {referenceField}
            {currencyField}
          </div>
        </>
      ) : (
        <>
          <div className="grid grid-cols-2 gap-4">
            {referenceField}
            {currencyField}
          </div>
          {nameField}
        </>
      )}

      {kindForm?.may_bear_interest && (
        <label
          htmlFor={`${idPrefix}-interest-bearing`}
          className="flex items-center gap-3 cursor-pointer group"
        >
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
      )}

      <div className="grid grid-cols-2 gap-4">
        {classOptions.length > 1 && (
          <SelectField
            label={t("asset.form_class_label")}
            id={`${idPrefix}-class`}
            name="class"
            value={formData.class}
            onChange={handleClassSelect}
            options={classOptions}
          />
        )}
        {categoryField}
      </div>

      <fieldset className="flex flex-col gap-1.5 border-none p-0 m-0">
        <legend className="m3-input-label">{t("asset.form_risk_label")}</legend>
        <div
          role="radiogroup"
          className="flex p-1 bg-m3-surface-variant rounded-2xl gap-1 overflow-hidden"
        >
          {RISK_LEVELS.map((level) => {
            const isSelected = formData.risk_level === level;

            return (
              <label
                key={level}
                id={`${idPrefix}-risk-${level}`}
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
