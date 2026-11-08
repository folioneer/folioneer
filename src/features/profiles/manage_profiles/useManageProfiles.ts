import { useCallback, useEffect, useState } from "react";
import type { Profile, ProfileSummary } from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { profileGateway } from "../gateway";
import { profileErrorToI18n } from "../shared/presenter";

/** The name dialog over the list: closed, adding a profile, or renaming one. */
type NameDialog = { open: false } | { open: true; profile: Profile | null };

/**
 * PFL-023 / PFL-030 / PFL-040 — the Profiles dialog: every profile with its account count,
 * read when it opens and again on `ProfileUpdated`, `AccountUpdated` and `SyncCompleted`.
 */
export function useManageProfiles(isOpen: boolean) {
  const [profiles, setProfiles] = useState<ProfileSummary[]>([]);
  const [error, setError] = useState<I18nMessage | null>(null);
  const [nameDialog, setNameDialog] = useState<NameDialog>({ open: false });
  // The profile awaiting the owner's confirmation before it is removed.
  const [pendingRemoval, setPendingRemoval] = useState<Profile | null>(null);

  const load = useCallback(async () => {
    const result = await profileGateway.listProfiles();
    if (result.status === "error") {
      logger.error("[useManageProfiles] the profiles could not be read", {
        code: result.error.code,
      });
      setError(profileErrorToI18n(result.error));
      return;
    }
    setProfiles(result.data.profiles);
    setError(null);
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    let isMounted = true;
    setError(null);
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
  }, [isOpen, load]);

  // PFL-040 — removed once confirmed; the core refuses a profile that holds an account, or
  // the default one.
  const confirmRemoval = async () => {
    if (!pendingRemoval) return;
    const profileId = pendingRemoval.id;
    setPendingRemoval(null);
    const result = await profileGateway.removeProfile(profileId);
    if (result.status === "error") {
      logger.warn("[useManageProfiles] the profile was not removed", { code: result.error.code });
      setError(profileErrorToI18n(result.error));
      return;
    }
    setError(null);
  };

  return {
    profiles,
    error,
    pendingRemoval,
    requestRemoval: (profile: Profile) => setPendingRemoval(profile),
    cancelRemoval: () => setPendingRemoval(null),
    confirmRemoval,
    nameDialog,
    openAdd: () => setNameDialog({ open: true, profile: null }),
    openRename: (profile: Profile) => setNameDialog({ open: true, profile }),
    closeNameDialog: () => setNameDialog({ open: false }),
  };
}
