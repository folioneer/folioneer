import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { Profile } from "@/bindings";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button";
import { TextField } from "@/ui/components/field/TextField";
import { Dialog } from "@/ui/components/modal/Dialog";
import { messageText } from "@/ui/format/i18n";
import { useProfileNameDialog } from "./useProfileNameDialog";

interface ProfileNameDialogProps {
  isOpen: boolean;
  /** The profile renamed; none adds a profile. */
  profile: Profile | null;
  onClose: () => void;
}

/** PFL-020 / PFL-030 — adds a profile, or renames one, by its name. */
export function ProfileNameDialog({ isOpen, profile, onClose }: ProfileNameDialogProps) {
  const { t } = useTranslation();
  const { name, setName, error, isSubmitting, submit } = useProfileNameDialog({
    isOpen,
    profile,
    onDone: onClose,
  });

  useEffect(() => {
    if (isOpen) logger.info("[ProfileNameDialog] mounted");
  }, [isOpen]);

  return (
    <Dialog
      id="profile-name-dialog"
      isOpen={isOpen}
      onClose={onClose}
      title={profile ? t("profile.name_dialog.rename_title") : t("profile.name_dialog.add_title")}
      actions={
        <>
          <Button id="profile-name-cancel" variant="tonal" onClick={onClose}>
            {t("action.cancel")}
          </Button>
          <Button
            id="profile-name-save"
            type="submit"
            form="profile-name-form"
            loading={isSubmitting}
          >
            {t("action.save")}
          </Button>
        </>
      }
    >
      <form id="profile-name-form" onSubmit={submit}>
        <TextField
          id="profile-name-input"
          label={t("profile.name_dialog.label")}
          value={name}
          onChange={(e) => setName(e.target.value)}
          error={messageText(t, error)}
          autoFocus
        />
      </form>
    </Dialog>
  );
}
