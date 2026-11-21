import { TrendingUp } from "lucide-react";

interface BrandMarkProps {
  /** `md` in the sidebar, `lg` on the startup screen (#873's mock). */
  size: "md" | "lg";
  /** The id of the name, for a screen that targets it. */
  nameId?: string;
}

const SIZES = {
  md: { box: "w-8 h-8 rounded-lg", icon: 20, name: "text-lg" },
  lg: { box: "w-12 h-12 rounded-xl shadow-elevation-2", icon: 28, name: "text-3xl" },
} as const;

/** The product's mark: the primary square with its trend icon, and the name beside it. */
export function BrandMark({ size, nameId }: BrandMarkProps) {
  const s = SIZES[size];
  return (
    <div className="flex items-center gap-3">
      <div className={`${s.box} bg-m3-primary flex items-center justify-center text-m3-on-primary`}>
        <TrendingUp size={s.icon} aria-hidden="true" />
      </div>
      {/* "Folioneer" is the fixed product brand name — intentionally not i18n'd */}
      <span id={nameId} className={`${s.name} font-bold text-m3-on-surface tracking-tight`}>
        Folioneer
      </span>
    </div>
  );
}
