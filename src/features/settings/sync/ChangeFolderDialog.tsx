import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { TextField } from "@/ui/components/field/TextField";
import { Dialog } from "@/ui/components/modal/Dialog";

interface ChangeFolderDialogProps {
  currentFolder: string;
  /** The core's refusal of the last folder submitted, already in the user's language. */
  error: string | undefined;
  /** Resolves to true when the core accepted the folder. */
  onSubmit: (folder: string) => Promise<boolean>;
  /** The native folder picker; resolves to the chosen path, or null when cancelled. */
  onBrowse: () => Promise<string | null>;
  onClose: () => void;
}

/** SYN-074 — designate another shared folder: typed, or picked with Browse. */
export function ChangeFolderDialog({
  currentFolder,
  error,
  onSubmit,
  onBrowse,
  onClose,
}: ChangeFolderDialogProps) {
  const { t } = useTranslation();
  const [folder, setFolder] = useState(currentFolder);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const browse = async () => {
    const picked = await onBrowse();
    if (picked !== null) setFolder(picked);
  };

  const submit = async () => {
    setIsSubmitting(true);
    const accepted = await onSubmit(folder);
    setIsSubmitting(false);
    if (accepted) onClose();
  };

  return (
    <Dialog
      id="sync-folder-dialog"
      isOpen
      onClose={onClose}
      title={t("sync.change_folder_prompt_title")}
      actions={
        <div className="flex items-center justify-end gap-3">
          <Button id="sync-folder-cancel" variant="ghost" onClick={onClose}>
            {t("action.cancel")}
          </Button>
          <Button
            id="sync-folder-submit"
            data-testid="sync-folder-submit"
            variant="primary"
            disabled={folder.trim() === "" || isSubmitting}
            loading={isSubmitting}
            onClick={() => void submit()}
          >
            {t("action.save")}
          </Button>
        </div>
      }
    >
      <div className="flex items-start gap-2">
        <div className="flex-1">
          <TextField
            id="sync-folder-value"
            label={t("sync.change_folder_prompt_label")}
            value={folder}
            error={error}
            onChange={(event) => setFolder(event.target.value)}
          />
        </div>
        <Button
          id="sync-folder-browse"
          data-testid="sync-folder-browse"
          type="button"
          variant="outline"
          className="mt-6"
          onClick={() => void browse()}
        >
          {t("sync.browse")}
        </Button>
      </div>
    </Dialog>
  );
}
