import { Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { BrandMark } from "./BrandMark";

/**
 * The screen shown while the window waits for the portfolio at launch, migrations included
 * (#873's mock): the brand over a faint watermark of its name, a small spinner beside the
 * message.
 */
export function StartupScreen() {
  const { t } = useTranslation();
  return (
    <div className="relative h-screen overflow-hidden flex items-center justify-center bg-m3-surface">
      {/* "Folioneer" is the fixed product brand name — intentionally not i18n'd */}
      <span
        id="startup-watermark"
        aria-hidden="true"
        className="pointer-events-none select-none absolute font-bold tracking-tight text-m3-primary opacity-[0.06] -rotate-12 whitespace-nowrap text-[220px]"
      >
        Folioneer
      </span>
      <div className="relative flex flex-col items-center gap-5">
        <BrandMark size="lg" nameId="startup-brand" />
        <div className="flex items-center gap-2" role="status">
          <Loader2 className="w-4 h-4 animate-spin text-m3-primary" aria-hidden="true" />
          <p id="startup-message" className="text-m3-on-surface-variant text-sm">
            {t("app.starting")}
          </p>
        </div>
      </div>
    </div>
  );
}
