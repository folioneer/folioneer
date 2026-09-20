import { useNavigate, useRouterState } from "@tanstack/react-router";
import { Info, Menu, Settings, TrendingUp, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { AboutModal } from "@/features/about";
import { logger } from "@/lib/logger";
import { useAppStore } from "@/lib/store";
import { IconButton } from "@/ui/components";
import { NAV_ITEMS } from "./navItems";

interface SidebarProps {
  isOpen: boolean;
  toggleDrawer: () => void;
}

export function Sidebar({ isOpen, toggleDrawer }: SidebarProps) {
  const { t } = useTranslation("common");
  const appVersion = useAppStore((state) => state.appVersion);
  const navigate = useNavigate();
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const [aboutOpen, setAboutOpen] = useState(false);

  useEffect(() => {
    logger.info("[Sidebar] mounted");
  }, []);

  return (
    <aside
      className={`
        relative z-50 h-full bg-m3-surface-container-high drawer-transition
        ${isOpen ? "w-80" : "w-20"}
        overflow-hidden flex flex-col shrink-0
      `}
    >
      <div className="p-4 flex items-center h-16">
        <IconButton
          variant="ghost"
          shape="round"
          size="md"
          icon={isOpen ? <X size={24} /> : <Menu size={24} />}
          aria-label={isOpen ? t("shell.sidebar_collapse") : t("shell.sidebar_expand")}
          onClick={toggleDrawer}
          className={!isOpen ? "mx-auto" : undefined}
        />
        {isOpen && (
          <div className="flex items-center gap-3 ml-2">
            <div className="w-8 h-8 bg-m3-primary rounded-lg flex items-center justify-center text-m3-on-primary">
              <TrendingUp size={20} />
            </div>
            {/* "Folioneer" is the fixed product brand name — intentionally not i18n'd */}
            <span className="text-lg font-bold text-m3-on-surface tracking-tight">Folioneer</span>
          </div>
        )}
      </div>

      <nav className="flex-1 px-3 mt-2 space-y-1">
        {NAV_ITEMS.map((item) => {
          const isActive = pathname === item.path || pathname.startsWith(`${item.path}/`);
          const label = t(item.labelKey);
          return (
            <button
              type="button"
              key={item.path}
              id={`nav-${item.path.slice(1).replace(/\//g, "-")}`}
              onClick={() => navigate({ to: item.path })}
              aria-label={label}
              className={`
                w-full m3-navigation-item
                ${isActive ? "m3-navigation-item-active" : ""}
                ${!isOpen && "justify-center px-0"}
              `}
            >
              <item.icon size={24} />
              {isOpen && <span>{label}</span>}
            </button>
          );
        })}

        <button
          type="button"
          id="nav-about"
          onClick={() => setAboutOpen(true)}
          aria-label={t("nav.about")}
          className={`w-full m3-navigation-item ${!isOpen && "justify-center px-0"}`}
        >
          <Info size={24} />
          {isOpen && <span>{t("nav.about")}</span>}
        </button>
      </nav>

      <nav aria-label={t("nav.settings")} className="px-3 pb-2">
        <button
          type="button"
          id="nav-settings"
          onClick={() => navigate({ to: "/settings" })}
          aria-label={t("nav.settings")}
          className={`
            w-full m3-navigation-item
            ${pathname === "/settings" ? "m3-navigation-item-active" : ""}
            ${!isOpen && "justify-center px-0"}
          `}
        >
          <Settings size={24} />
          {isOpen && <span>{t("nav.settings")}</span>}
        </button>
      </nav>

      <div className="px-3 py-4 flex flex-col items-center justify-center min-h-12">
        <span
          className={`
            font-mono text-[14px] tracking-tight transition-opacity duration-300
            ${isOpen ? "opacity-60" : "opacity-40"}
          `}
        >
          {isOpen ? t("shell.sidebar_version", { version: appVersion }) : `v${appVersion}`}
        </span>
      </div>

      <AboutModal isOpen={aboutOpen} onClose={() => setAboutOpen(false)} />
    </aside>
  );
}
