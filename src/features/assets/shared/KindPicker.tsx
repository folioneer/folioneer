import { useTranslation } from "react-i18next";
import type { AssetKind } from "@/bindings";
import { KIND_FORMS } from "./creationDefaults";
import { assetKindLabelKey, assetKindSaysKey } from "./presenter";

interface KindPickerProps {
  value: AssetKind;
  onChange: (kind: AssetKind) => void;
  idPrefix: string;
}

/**
 * AST-040 — the choice of an asset's kind, with one line saying what the chosen kind
 * means. The kinds offered are the core's (`KIND_FORMS`); cash is never among them. Each
 * kind is a button of its own, so a user, a keyboard and a test all press the same control.
 */
export function KindPicker({ value, onChange, idPrefix }: KindPickerProps) {
  const { t } = useTranslation();
  const labelId = `${idPrefix}-kind-label`;

  return (
    <div className="flex flex-col gap-1.5">
      <span id={labelId} className="m3-input-label">
        {t("asset.form_kind_label")}
      </span>
      <div
        role="radiogroup"
        aria-labelledby={labelId}
        aria-describedby={`${idPrefix}-kind-says`}
        className="flex p-1 bg-m3-surface-variant rounded-2xl gap-1 overflow-hidden"
      >
        {KIND_FORMS.map(({ kind }) => {
          const isSelected = kind === value;
          return (
            // biome-ignore lint/a11y/useSemanticElements: a native radio would be hidden behind its label, and a hidden control is one a test cannot press (F29)
            <button
              key={kind}
              type="button"
              role="radio"
              aria-checked={isSelected}
              id={`${idPrefix}-kind-${kind}`}
              onClick={() => onChange(kind)}
              className={`
                flex-1 flex items-center justify-center py-2 rounded-xl
                text-sm font-bold cursor-pointer transition-all duration-200
                focus-visible:ring-2 focus-visible:ring-m3-primary focus-visible:ring-offset-1
                ${
                  isSelected
                    ? "bg-m3-primary text-m3-on-primary"
                    : "text-m3-on-surface-variant hover:bg-m3-primary/10"
                }
              `}
            >
              {t(assetKindLabelKey(kind))}
            </button>
          );
        })}
      </div>
      <p id={`${idPrefix}-kind-says`} className="text-xs text-m3-on-surface-variant ml-1">
        {t(assetKindSaysKey(value))}
      </p>
    </div>
  );
}
