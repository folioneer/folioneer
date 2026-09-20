import { ArrowLeft } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { IconButton } from "@/ui/components/button/IconButton";
import { PriceFreshnessIndicator } from "./price_freshness/PriceFreshnessIndicator";
import { SyncIndicator } from "./sync_indicator/SyncIndicator";
import { ThemeToggle } from "./theme_toggle/ThemeToggle";
import { useHeaderConfig } from "./useHeaderConfig";

export function Header() {
  const { t } = useTranslation();
  const { title, onBack } = useHeaderConfig();

  useEffect(() => {
    logger.info("[Header] mounted");
  }, []);

  // text-white is intentional: lives exclusively on the fixed-brand indigo gradient
  // (--color-header-from/to). White is always accessible on rich indigo (WCAG AA).
  return (
    <header
      className="
        bg-linear-to-br from-header-from to-header-to
        text-white px-6
        flex items-center gap-4
        h-app-bar shrink-0
        relative z-50
        shadow-elevation-1
      "
    >
      {onBack && (
        <IconButton
          icon={<ArrowLeft size={20} />}
          onClick={onBack}
          aria-label={t("action.back")}
          className="text-white hover:enabled:bg-white/20"
        />
      )}
      <div className="flex-1">
        <h1 className="text-lg font-semibold leading-tight">{title}</h1>
      </div>
      <SyncIndicator />
      <PriceFreshnessIndicator />
      <ThemeToggle />
    </header>
  );
}
