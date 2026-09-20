import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { Account } from "@/bindings";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { AccountForm } from "../shared/AccountForm";
import { useEditAccountModal } from "./useEditAccountModal";

interface EditAccountModalProps {
  isOpen: boolean;
  onClose: () => void;
  account: Account | null;
}

export function EditAccountModal({ isOpen, onClose, account }: EditAccountModalProps) {
  const { t } = useTranslation();
  useEffect(() => {
    logger.info("[EditAccountModal] mounted");
  }, []);

  const { formData, error, isSubmitting, handleChange, handleSubmit, frequencies } =
    useEditAccountModal({ account, onClose });

  const actions = (
    <>
      <Button variant="secondary" onClick={onClose}>
        {t("action.cancel")}
      </Button>
      <Button
        type="submit"
        form="edit-account-form"
        variant="primary"
        loading={isSubmitting}
        disabled={isSubmitting || formData.name.trim().length === 0}
      >
        {t("action.save")}
      </Button>
    </>
  );

  return (
    <Dialog
      id="edit-account-dialog"
      isOpen={isOpen}
      onClose={onClose}
      title={t("account.edit_modal_title")}
      actions={actions}
      maxWidth="max-w-xl"
    >
      <form id="edit-account-form" className="py-2" onSubmit={handleSubmit}>
        <AccountForm
          formData={formData}
          handleChange={handleChange}
          frequencies={frequencies}
          idPrefix="edit-account"
        />
        {/* R13 — inline error, modal stays open */}
        {error && (
          <p role="alert" className="mt-3 text-sm text-m3-error">
            {t(error.key, error.vars)}
          </p>
        )}
      </form>
    </Dialog>
  );
}
