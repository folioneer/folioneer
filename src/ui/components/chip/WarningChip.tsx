import { AlertTriangle } from "lucide-react";

interface WarningChipProps {
  id: string;
  /** Already translated: the chip renders text, never a key. */
  label: string;
}

/** A small rounded warning: an icon and a short count or state, in the warning colours. */
export function WarningChip({ id, label }: WarningChipProps) {
  return (
    <span
      id={id}
      data-testid={id}
      className="inline-flex items-center gap-1 whitespace-nowrap rounded-full bg-m3-warning-container px-2.5 py-1 text-xs font-medium text-m3-on-warning-container"
    >
      <AlertTriangle size={12} aria-hidden="true" />
      {label}
    </span>
  );
}
