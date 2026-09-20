import { useMemo, useState } from "react";
import type { AssetClass, AssetLookupResult, Exchange } from "@/bindings";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";
import { DEFAULT_RISK_BY_CLASS, SYSTEM_CATEGORY_ID } from "../shared/constants";
import { hasDuplicateReference } from "../shared/validateAsset";
import { useAssets } from "../useAssets";

interface UseAddAssetProps {
  onSubmitSuccess?: (assetId: string) => void;
  prefill?: AssetLookupResult;
}

export function useAddAsset({ onSubmitSuccess, prefill }: UseAddAssetProps = {}) {
  const { addAsset, assets } = useAssets();
  const categories = useAppStore((s) => s.categories);

  // CSH-015 — default class is `Stocks` (the dropdown's first user-selectable
  // entry). `Cash` is reserved for the system Cash Asset and never available here.
  const [formData, setFormData] = useState<{
    name: string;
    reference: string;
    isin: string;
    class: AssetClass;
    currency: string;
    risk_level: number;
    category_id: string;
    exchange: Exchange | null;
    interest_bearing: boolean;
  }>({
    name: prefill?.name ?? "",
    reference: prefill?.reference ?? "",
    isin: prefill?.isin ?? "",
    class: (prefill?.asset_class ?? "Stocks") as AssetClass,
    currency: prefill?.currency ?? "EUR",
    risk_level: prefill?.asset_class
      ? DEFAULT_RISK_BY_CLASS[prefill.asset_class as AssetClass]
      : DEFAULT_RISK_BY_CLASS.Stocks,
    category_id: SYSTEM_CATEGORY_ID,
    exchange: prefill?.exchange ?? null,
    interest_bearing: false,
  });
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Duplicate reference warning — R9 (includes archived assets)
  const duplicateWarning = useMemo(
    () => hasDuplicateReference(formData.reference, assets),
    [formData.reference, assets],
  );

  const handleChange = (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
    const { name, value, type } = e.target;
    setFormData((prev) => ({
      ...prev,
      [name]:
        type === "checkbox" && "checked" in e.target
          ? e.target.checked
          : name === "risk_level"
            ? parseInt(value, 10)
            : value,
    }));
  };

  // Auto-fill risk_level when class changes — R10 (creation only)
  const handleClassChange = (assetClass: AssetClass) => {
    setFormData((prev) => ({
      ...prev,
      class: assetClass,
      risk_level: DEFAULT_RISK_BY_CLASS[assetClass],
    }));
  };

  // AST-021 — exchange picker change handler
  const handleExchangeChange = (exchange: Exchange | null) => {
    setFormData((prev) => ({ ...prev, exchange }));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setIsSubmitting(true);

    const result = await addAsset({
      name: formData.name,
      reference: formData.reference,
      isin: formData.isin.trim() ? formData.isin.trim() : null,
      class: formData.class,
      currency: formData.currency,
      risk_level: formData.risk_level,
      category_id: formData.category_id || SYSTEM_CATEGORY_ID,
      exchange: formData.exchange,
      interest_bearing: formData.interest_bearing,
    });

    setIsSubmitting(false);

    if (result.error) {
      setError(result.error);
      return;
    }

    if (onSubmitSuccess && result.data) {
      onSubmitSuccess(result.data.id);
    }

    setFormData({
      name: "",
      reference: "",
      isin: "",
      class: "Stocks",
      currency: "EUR",
      risk_level: DEFAULT_RISK_BY_CLASS.Stocks,
      category_id: SYSTEM_CATEGORY_ID,
      exchange: null,
      interest_bearing: false,
    });
  };

  return {
    formData,
    error,
    isSubmitting,
    duplicateWarning,
    handleChange,
    handleClassChange,
    handleExchangeChange,
    handleSubmit,
    categories,
  };
}
