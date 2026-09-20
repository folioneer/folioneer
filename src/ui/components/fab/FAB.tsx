import { Plus } from "lucide-react";
import type { ReactNode } from "react";

interface FABProps {
  onClick: () => void;
  label: string;
  icon?: ReactNode;
  id?: string;
}

/**
 * FAB — Floating Action Button (M3 standard, 56×56px)
 *
 * Fixed bottom-right position for desktop layouts.
 * Generic: pass a custom icon or default to Plus.
 */
export function FAB({ onClick, label, icon, id }: FABProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      id={id}
      aria-label={label}
      title={label}
      className="fixed bottom-12 right-12 w-14 h-14 rounded-full bg-m3-primary text-m3-on-primary hover:bg-m3-primary/90 shadow-elevation-3 hover:shadow-elevation-4 active:shadow-elevation-3 transition-all duration-200 flex items-center justify-center z-40"
    >
      {icon ?? <Plus size={24} strokeWidth={2.5} />}
    </button>
  );
}
