import { Check, ChevronDown, Pencil, Plus, User } from "lucide-react";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { ManageProfilesDialog, ProfileNameDialog } from "@/features/profiles";
import { logger } from "@/lib/logger";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { chipProfileName } from "./presenter";
import { useProfileChip } from "./useProfileChip";

/**
 * PFL-012 / PFL-032 — the profile in use, at the header's far right, always shown (#767,
 * the validated mock): shell chrome, hosting the profiles feature's dialogs (F26, F31). With one profile its menu adds one; with several it switches and
 * opens the Profiles dialog. Listens through `useProfileChip`.
 */
export function ProfileChip() {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const {
    profiles,
    inUse,
    menuOpen,
    toggleMenu,
    closeMenu,
    choose,
    dialog,
    openDialog,
    closeDialog,
    error,
    clearError,
  } = useProfileChip();

  useEffect(() => {
    logger.info("[ProfileChip] mounted");
  }, []);

  useEffect(() => {
    if (error) {
      showSnackbar(t(error.key, error.vars), "error");
      clearError();
    }
  }, [error, showSnackbar, t, clearError]);

  // Keyboard: the menu takes the focus when it opens and gives it back to the chip on close.
  const wrapper = useRef<HTMLDivElement>(null);
  const wasOpen = useRef(false);
  useEffect(() => {
    if (menuOpen) {
      wrapper.current?.querySelector<HTMLButtonElement>("#profile-chip-menu button")?.focus();
    } else if (wasOpen.current) {
      wrapper.current?.querySelector<HTMLButtonElement>("#profile-chip")?.focus();
    }
    wasOpen.current = menuOpen;
  }, [menuOpen]);

  const defaultName = t("profile.default_name");
  const inUseName = chipProfileName(inUse, defaultName);
  // Nothing until the profiles are read, so another profile's name never flashes as "Me".
  if (!inUse) return null;

  // Raw white tokens are intentional: the chip lives on the fixed-brand indigo header
  // gradient, which never changes in dark mode (see ThemeToggle).
  return (
    <div ref={wrapper} className="relative">
      <button
        type="button"
        id="profile-chip"
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        aria-label={t("profile.chip.label", { name: inUseName })}
        onClick={toggleMenu}
        onKeyDown={(e) => {
          if (e.key === "Escape" && menuOpen) closeMenu();
        }}
        className="
          flex items-center gap-1.5 h-9 px-3
          rounded-full bg-white/15 text-white text-sm font-medium
          hover:bg-white/25 transition-colors
          focus-visible:outline-2 focus-visible:outline-white focus-visible:outline-offset-2
        "
      >
        <User size={16} />
        <span className="max-w-40 truncate">{inUseName}</span>
        <ChevronDown size={16} />
      </button>

      {menuOpen && (
        <>
          <button
            type="button"
            aria-hidden="true"
            tabIndex={-1}
            className="fixed inset-0 cursor-default bg-transparent border-none"
            onClick={closeMenu}
          />
          <div
            id="profile-chip-menu"
            role="menu"
            tabIndex={-1}
            onKeyDown={(e) => {
              if (e.key === "Escape") closeMenu();
            }}
            className="
              absolute right-0 top-full mt-2 min-w-56 py-2
              rounded-2xl bg-m3-surface-container text-m3-on-surface shadow-elevation-3 z-50
            "
          >
            {profiles.length > 1 ? (
              <>
                {profiles.map((summary) => {
                  const active = summary.profile.id === inUse?.id;
                  return (
                    <button
                      key={summary.profile.id}
                      type="button"
                      role="menuitemradio"
                      aria-checked={active}
                      id={`profile-chip-choose-${summary.profile.id}`}
                      onClick={() => void choose(summary.profile.id)}
                      className="flex w-full items-center gap-3 px-4 py-2 text-sm hover:bg-m3-on-surface/5"
                    >
                      <span className="w-4">{active && <Check size={16} />}</span>
                      <span className="truncate">
                        {chipProfileName(summary.profile, defaultName)}
                      </span>
                    </button>
                  );
                })}
                <div className="my-1 border-t border-m3-outline-variant" />
                <button
                  type="button"
                  role="menuitem"
                  id="profile-chip-manage"
                  onClick={() => openDialog("manage")}
                  className="flex w-full items-center gap-3 px-4 py-2 text-sm hover:bg-m3-on-surface/5"
                >
                  <Pencil size={16} />
                  {t("profile.chip.manage")}
                </button>
              </>
            ) : (
              <button
                type="button"
                role="menuitem"
                id="profile-chip-add"
                onClick={() => openDialog("add")}
                className="flex w-full items-center gap-3 px-4 py-2 text-sm hover:bg-m3-on-surface/5"
              >
                <Plus size={16} />
                {t("profile.chip.add")}
              </button>
            )}
          </div>
        </>
      )}

      <ProfileNameDialog isOpen={dialog === "add"} profile={null} onClose={closeDialog} />
      <ManageProfilesDialog isOpen={dialog === "manage"} onClose={closeDialog} />
    </div>
  );
}
