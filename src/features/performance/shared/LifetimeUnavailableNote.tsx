import { Info } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { I18nMessage } from "@/ui/format/i18n";

interface LifetimeUnavailableNoteProps {
  /** Why the lifetime metrics are absent (PRF-088); nothing renders when null. */
  note: I18nMessage | null;
  /** Base of the element id, so two pages never emit colliding ids. */
  idPrefix: string;
}

/** PRF-088 — a persistent note above the performance table naming why lifetime metrics are absent. */
export function LifetimeUnavailableNote({ note, idPrefix }: LifetimeUnavailableNoteProps) {
  const { t } = useTranslation();
  if (note === null) return null;
  return (
    <div
      id={`${idPrefix}-lifetime-note`}
      role="note"
      className="flex items-start gap-3 rounded-2xl bg-m3-surface-container-high px-4 py-3 text-sm text-m3-on-surface"
    >
      <Info size={18} className="mt-0.5 shrink-0 text-m3-primary" aria-hidden="true" />
      <p>{t(note.key, note.vars)}</p>
    </div>
  );
}
