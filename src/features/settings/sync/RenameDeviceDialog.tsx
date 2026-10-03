import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { TextField } from "@/ui/components/field/TextField";
import { Dialog } from "@/ui/components/modal/Dialog";

interface RenameDeviceDialogProps {
  currentName: string;
  /** The core's refusal of the last name submitted, already in the user's language. */
  error: string | undefined;
  /** Resolves to true when the core accepted the name. */
  onSubmit: (deviceName: string) => Promise<boolean>;
  onClose: () => void;
}

/** SYN-072 — rename this computer: one name, saved once the core accepts it. */
export function RenameDeviceDialog({
  currentName,
  error,
  onSubmit,
  onClose,
}: RenameDeviceDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(currentName);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const submit = async () => {
    setIsSubmitting(true);
    const accepted = await onSubmit(name);
    setIsSubmitting(false);
    if (accepted) onClose();
  };

  return (
    <Dialog
      id="sync-rename-dialog"
      isOpen
      onClose={onClose}
      title={t("sync.rename_prompt_title")}
      actions={
        <div className="flex items-center justify-end gap-3">
          <Button id="sync-rename-cancel" variant="ghost" onClick={onClose}>
            {t("action.cancel")}
          </Button>
          <Button
            id="sync-rename-submit"
            data-testid="sync-rename-submit"
            variant="primary"
            disabled={name.trim() === "" || isSubmitting}
            loading={isSubmitting}
            onClick={() => void submit()}
          >
            {t("action.save")}
          </Button>
        </div>
      }
    >
      <TextField
        id="sync-rename-value"
        label={t("sync.rename_prompt_label")}
        value={name}
        error={error}
        onChange={(event) => setName(event.target.value)}
      />
    </Dialog>
  );
}
