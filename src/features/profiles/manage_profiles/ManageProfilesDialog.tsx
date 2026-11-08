import { Pencil, Plus, Trash2, User } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { Button, IconButton } from "@/ui/components/button";
import { ConfirmationDialog, Dialog } from "@/ui/components/modal/Dialog";
import { ProfileNameDialog } from "../profile_name_dialog/ProfileNameDialog";
import { accountCountMessage, profileDisplayName } from "../shared/presenter";
import { useManageProfiles } from "./useManageProfiles";

interface ManageProfilesDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * PFL-023 / PFL-030 / PFL-040 — every profile with its account count, renamed or removed
 * (only one holding no account, never the default one), and a profile added.
 */
export function ManageProfilesDialog({ isOpen, onClose }: ManageProfilesDialogProps) {
  const { t } = useTranslation();
  const {
    profiles,
    error,
    pendingRemoval,
    requestRemoval,
    cancelRemoval,
    confirmRemoval,
    nameDialog,
    openAdd,
    openRename,
    closeNameDialog,
  } = useManageProfiles(isOpen);

  useEffect(() => {
    if (isOpen) logger.info("[ManageProfilesDialog] mounted");
  }, [isOpen]);

  const defaultName = t("profile.default_name");

  return (
    <>
      <Dialog
        id="manage-profiles-dialog"
        isOpen={isOpen}
        onClose={onClose}
        title={t("profile.manage.title")}
        actions={
          <Button id="manage-profiles-close" variant="secondary" onClick={onClose}>
            {t("action.close")}
          </Button>
        }
      >
        <ul className="flex flex-col divide-y divide-m3-outline-variant">
          {profiles.map((summary) => {
            const name = profileDisplayName(summary.profile, defaultName);
            const count = accountCountMessage(summary.account_count);
            return (
              <li
                key={summary.profile.id}
                id={`manage-profiles-row-${summary.profile.id}`}
                className="flex items-center gap-3 py-3"
              >
                <User size={18} className="text-m3-on-surface-variant shrink-0" />
                <span className="flex-1 font-medium text-m3-on-surface truncate">{name}</span>
                <span className="text-sm text-m3-on-surface-variant">
                  {t(count.key, count.vars)}
                </span>
                <IconButton
                  id={`manage-profiles-rename-${summary.profile.id}`}
                  icon={<Pencil size={18} />}
                  aria-label={t("profile.manage.rename", { name })}
                  onClick={() => openRename(summary.profile)}
                />
                <IconButton
                  id={`manage-profiles-remove-${summary.profile.id}`}
                  icon={<Trash2 size={18} />}
                  aria-label={t("profile.manage.remove", { name })}
                  disabled={!summary.removable}
                  onClick={() => requestRemoval(summary.profile)}
                />
              </li>
            );
          })}
        </ul>
        <p className="mt-3 text-xs text-m3-on-surface-variant">{t("profile.manage.remove_hint")}</p>
        {error && (
          <p id="manage-profiles-error" role="alert" className="mt-2 text-sm text-m3-error">
            {t(error.key, error.vars)}
          </p>
        )}
        <div className="mt-4">
          <Button
            id="manage-profiles-add"
            variant="outline"
            icon={<Plus size={18} />}
            onClick={openAdd}
          >
            {t("profile.manage.add")}
          </Button>
        </div>
      </Dialog>
      <ConfirmationDialog
        isOpen={pendingRemoval !== null}
        onCancel={cancelRemoval}
        onConfirm={() => void confirmRemoval()}
        title={t("profile.manage.remove_confirm_title")}
        message={t("profile.manage.remove_confirm_message", {
          name: pendingRemoval ? profileDisplayName(pendingRemoval, defaultName) : "",
        })}
        confirmLabel={t("action.delete")}
        cancelLabel={t("action.cancel")}
        variant="danger"
        confirmId="manage-profiles-remove-confirm"
      />
      <ProfileNameDialog
        isOpen={nameDialog.open}
        profile={nameDialog.open ? nameDialog.profile : null}
        onClose={closeNameDialog}
      />
    </>
  );
}
