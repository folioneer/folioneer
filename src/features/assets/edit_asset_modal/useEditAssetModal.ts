import { useEffect, useState } from "react";
import type { Asset, AssetClass, AssetKind, Exchange } from "@/bindings";
import { logger } from "@/lib/logger";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";
import { classForKind, DEFAULT_ASSET_KIND, kindFormOf } from "../shared/creationDefaults";
import { useAssets } from "../useAssets";

interface UseEditAssetModalProps {
  asset: Asset | null;
  onClose: () => void;
}

export function useEditAssetModal({ asset, onClose }: UseEditAssetModalProps) {
  const { updateAsset } = useAssets();
  const categories = useAppStore((s) => s.categories);

  const [formData, setFormData] = useState<{
    kind: AssetKind;
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
    kind: DEFAULT_ASSET_KIND,
    name: "",
    reference: "",
    isin: "",
    class: "Stocks",
    currency: "USD",
    risk_level: 3,
    category_id: "",
    exchange: null,
    interest_bearing: false,
  });
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Sync form data when asset changes
  useEffect(() => {
    if (asset) {
      setFormData({
        kind: asset.kind,
        name: asset.name,
        reference: asset.reference,
        isin: asset.isin ?? "",
        class: asset.class,
        currency: asset.currency,
        risk_level: asset.risk_level,
        category_id: asset.category.id,
        exchange: asset.exchange,
        interest_bearing: asset.interest_bearing,
      });
      setError(null);
    }
  }, [asset]);

  // AST-031 — an edit may change the kind, to settle an asset its kind's rules refuse; a
  // class the new kind does not offer gives way to the kind's preselected one.
  const handleKindChange = (kind: AssetKind) => {
    setFormData((prev) => ({ ...prev, kind, class: classForKind(kind, prev.class) }));
  };

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

  // R12: class change in edit mode does NOT auto-fill risk_level
  const handleClassChange = (_assetClass: AssetClass) => {
    // intentionally a no-op — risk_level suggestion only applies at creation (R10)
  };

  // AST-022 — exchange picker change handler (freely set/change/clear)
  const handleExchangeChange = (exchange: Exchange | null) => {
    setFormData((prev) => ({ ...prev, exchange }));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!asset) return;

    setError(null);
    setIsSubmitting(true);
    const kindForm = kindFormOf(formData.kind);
    const result = await updateAsset({
      kind: formData.kind,
      asset_id: asset.id,
      name: formData.name,
      reference: formData.reference,
      isin: kindForm?.has_isin && formData.isin.trim() ? formData.isin.trim() : null,
      class: formData.class,
      currency: formData.currency,
      risk_level: formData.risk_level,
      category_id: formData.category_id,
      exchange: kindForm?.has_exchange ? formData.exchange : null,
      interest_bearing: !!kindForm?.may_bear_interest && formData.interest_bearing,
    });

    setIsSubmitting(false);

    if (result.error) {
      logger.error("[useEditAssetModal] update failed", {
        error: result.error,
      });
      setError(result.error);
      return;
    }

    onClose();
  };

  return {
    formData,
    error,
    isSubmitting,
    handleChange,
    handleKindChange,
    handleClassChange,
    handleExchangeChange,
    handleSubmit,
    categories,
  };
}
