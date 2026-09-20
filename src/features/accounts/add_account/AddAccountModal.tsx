import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { AccountForm } from "../shared/AccountForm";
import { useAddAccount } from "./useAddAccount";

interface AddAccountModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function AddAccountModal({ isOpen, onClose }: AddAccountModalProps) {
  const { t } = useTranslation();
  useEffect(() => {
    logger.info("[AddAccountModal] mounted");
  }, []);

  const { formData, error, isSubmitting, handleChange, handleSubmit, frequencies } = useAddAccount({
    onSubmitSuccess: onClose,
  });

  const actions = (
    <>
      <Button variant="secondary" onClick={onClose}>
        {t("action.cancel")}
      </Button>
      <Button
        type="submit"
        form="add-account-form"
        variant="primary"
        loading={isSubmitting}
        disabled={isSubmitting || formData.name.trim().length === 0}
      >
        {t("action.add")}
      </Button>
    </>
  );

  return (
    <Dialog
      id="add-account-dialog"
      isOpen={isOpen}
      onClose={onClose}
      title={t("account.add_modal_title")}
      actions={actions}
    >
      <form id="add-account-form" className="py-2" onSubmit={handleSubmit}>
        <AccountForm
          formData={formData}
          handleChange={handleChange}
          frequencies={frequencies}
          idPrefix="add-account"
        />
        {/* R13 — inline error stays modal open */}
        {error && (
          <p role="alert" className="mt-3 text-sm text-m3-error">
            {t(error.key, error.vars)}
          </p>
        )}
      </form>
    </Dialog>
  );
}
