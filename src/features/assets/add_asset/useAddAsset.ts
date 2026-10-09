import { useEffect, useState } from "react";
import type { AssetClass, AssetKind, AssetLookupResult, Exchange } from "@/bindings";
import { logger } from "@/lib/logger";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";
import { assetGateway } from "../gateway";
import {
  classForKind,
  DEFAULT_CATEGORY_ID,
  defaultRiskOf,
  kindFormOf,
} from "../shared/creationDefaults";
import { useAssets } from "../useAssets";

interface UseAddAssetProps {
  /** AST-040 — the kind chosen, held by whoever shows the form. */
  kind: AssetKind;
  onSubmitSuccess?: (assetId: string) => void;
  prefill?: AssetLookupResult;
}

interface AddAssetFields {
  name: string;
  reference: string;
  isin: string;
  class: AssetClass;
  currency: string;
  risk_level: number;
  category_id: string;
  exchange: Exchange | null;
  interest_bearing: boolean;
}

function emptyFields(kind: AssetKind, prefill?: AssetLookupResult): AddAssetFields {
  const assetClass = classForKind(kind, prefill?.asset_class);
  return {
    name: prefill?.name ?? "",
    reference: prefill?.reference ?? "",
    isin: prefill?.isin ?? "",
    class: assetClass,
    currency: prefill?.currency ?? "EUR",
    risk_level: defaultRiskOf(assetClass),
    category_id: DEFAULT_CATEGORY_ID,
    exchange: prefill?.exchange ?? null,
    interest_bearing: false,
  };
}

/**
 * The state of the "New asset" form. What the form of each kind asks for is the core's
 * (AST-037): this hook holds what the user typed and sends only the fields the kind has.
 */
export function useAddAsset({ kind, onSubmitSuccess, prefill }: UseAddAssetProps) {
  const { addAsset } = useAssets();
  const categories = useAppStore((s) => s.categories);
  const kindForm = kindFormOf(kind);

  const [fields, setFields] = useState<AddAssetFields>(() => emptyFields(kind, prefill));
  const [referenceTyped, setReferenceTyped] = useState(!!prefill?.reference);
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // AST-037 — a kind offers its own classes: a class the new kind does not offer gives way
  // to the kind's preselected one, with that class's risk level (R10). AST-038 — a
  // reference that was only proposed does not follow the asset to a kind that proposes none.
  useEffect(() => {
    const proposes = !!kindFormOf(kind)?.proposes_reference;
    setFields((prev) => {
      const assetClass = classForKind(kind, prev.class);
      const reference = proposes || referenceTyped ? prev.reference : "";
      return assetClass === prev.class && reference === prev.reference
        ? prev
        : {
            ...prev,
            reference,
            class: assetClass,
            risk_level: assetClass === prev.class ? prev.risk_level : defaultRiskOf(assetClass),
          };
    });
    setError(null);
  }, [kind, referenceTyped]);

  // AST-038 — until the user types a reference, a custom asset's is proposed from its name
  // by the core.
  const proposesReference = !!kindForm?.proposes_reference && !referenceTyped;
  const { name } = fields;
  useEffect(() => {
    if (!proposesReference) return;
    let isCurrent = true;
    assetGateway
      .proposeAssetReference(name)
      .then((reference) => {
        if (isCurrent) setFields((prev) => ({ ...prev, reference }));
      })
      .catch((e) => logger.error("[useAddAsset] no reference proposed", { error: e }));
    return () => {
      isCurrent = false;
    };
  }, [proposesReference, name]);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
    const { name: field, value, type } = e.target;
    if (field === "reference") setReferenceTyped(true);
    setFields((prev) => ({
      ...prev,
      [field]:
        type === "checkbox" && "checked" in e.target
          ? e.target.checked
          : field === "risk_level"
            ? parseInt(value, 10)
            : value,
    }));
  };

  // Auto-fill risk_level when class changes — R10 (creation only)
  const handleClassChange = (assetClass: AssetClass) => {
    setFields((prev) => ({
      ...prev,
      class: assetClass,
      risk_level: defaultRiskOf(assetClass),
    }));
  };

  // AST-021 — exchange picker change handler
  const handleExchangeChange = (exchange: Exchange | null) => {
    setFields((prev) => ({ ...prev, exchange }));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setIsSubmitting(true);

    const result = await addAsset({
      kind,
      name: fields.name,
      reference: fields.reference,
      isin: kindForm?.has_isin && fields.isin.trim() ? fields.isin.trim() : null,
      class: fields.class,
      currency: fields.currency,
      risk_level: fields.risk_level,
      category_id: fields.category_id || DEFAULT_CATEGORY_ID,
      exchange: kindForm?.has_exchange ? fields.exchange : null,
      interest_bearing: !!kindForm?.may_bear_interest && fields.interest_bearing,
    });

    setIsSubmitting(false);

    if (result.error) {
      setError(result.error);
      return;
    }

    if (onSubmitSuccess && result.data) {
      onSubmitSuccess(result.data.id);
    }

    setFields(emptyFields(kind));
    setReferenceTyped(false);
  };

  return {
    formData: { ...fields, kind },
    referenceProposed: proposesReference && fields.reference !== "",
    error,
    isSubmitting,
    handleChange,
    handleClassChange,
    handleExchangeChange,
    handleSubmit,
    categories,
  };
}
