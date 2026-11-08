import { useNavigate } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import type { ProfileList } from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { profileGateway } from "../gateway";
import { profileErrorToI18n } from "../shared/presenter";

/** Which dialog the chip opened, if any. */
type ProfileDialog = "add" | "manage" | null;

/**
 * PFL-012 / PFL-023 / PFL-032 — the header's profile chip: the profiles and the one in use,
 * read on mount and again on `ProfileUpdated`, `AccountUpdated` (the counts) and
 * `SyncCompleted` (another device's changes, SYN-064).
 */
export function useProfileChip() {
  const navigate = useNavigate();
  const [list, setList] = useState<ProfileList | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [dialog, setDialog] = useState<ProfileDialog>(null);
  const [error, setError] = useState<I18nMessage | null>(null);

  const load = useCallback(async () => {
    const result = await profileGateway.listProfiles();
    if (result.status === "error") {
      logger.error("[useProfileChip] the profiles could not be read", { code: result.error.code });
      setError(profileErrorToI18n(result.error));
      return;
    }
    setList(result.data);
  }, []);

  useEffect(() => {
    let isMounted = true;
    void load();
    const unlisten = profileGateway.subscribeToEvents((type) => {
      if (!isMounted) return;
      if (type === "ProfileUpdated" || type === "AccountUpdated" || type === "SyncCompleted") {
        void load();
      }
    });
    return () => {
      isMounted = false;
      void unlisten.then((stop) => stop());
    };
  }, [load]);

  // PFL-032 / PFL-032A — another profile becomes the one in use; the window returns to the
  // account list, which the new profile's accounts fill.
  const choose = async (profileId: string) => {
    setMenuOpen(false);
    if (list?.in_use === profileId) return;
    const result = await profileGateway.setProfileInUse(profileId);
    if (result.status === "error") {
      logger.warn("[useProfileChip] the profile was not chosen", { code: result.error.code });
      setError(profileErrorToI18n(result.error));
      return;
    }
    void navigate({ to: "/accounts" });
  };

  const openDialog = (next: Exclude<ProfileDialog, null>) => {
    setMenuOpen(false);
    setDialog(next);
  };

  const inUse = list?.profiles.find((summary) => summary.profile.id === list.in_use) ?? null;

  return {
    profiles: list?.profiles ?? [],
    inUse: inUse?.profile ?? null,
    menuOpen,
    toggleMenu: () => setMenuOpen((open) => !open),
    closeMenu: () => setMenuOpen(false),
    choose,
    dialog,
    openDialog,
    closeDialog: () => setDialog(null),
    error,
    clearError: () => setError(null),
  };
}
