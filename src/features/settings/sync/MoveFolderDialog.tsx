import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";

interface MoveFolderDialogProps {
  /** The folder chosen, which holds no portfolio. */
  folder: string;
  /** Resolves once the move was answered. */
  onMove: () => Promise<void>;
  onKeep: () => void;
}

/**
 * SYN-075 — moving the portfolio to a folder that holds none is a new beginning:
 * every other computer rejoins from a fresh installation. Keeping the current folder is the
 * filled action in focus.
 */
export function MoveFolderDialog({ folder, onMove, onKeep }: MoveFolderDialogProps) {
  const { t } = useTranslation();
  const [isMoving, setIsMoving] = useState(false);
  const move = async () => {
    setIsMoving(true);
    try {
      await onMove();
    } finally {
      setIsMoving(false);
    }
  };
  return (
    <Dialog
      id="sync-move-dialog"
      isOpen
      onClose={onKeep}
      disableClose={isMoving}
      title={t("sync.move_confirm_title")}
      maxWidth="max-w-lg"
      actions={
        <>
          <Button
            id="sync-move-confirm"
            variant="outline"
            disabled={isMoving}
            loading={isMoving}
            onClick={() => void move()}
          >
            {t("sync.move_confirm_move")}
          </Button>
          <Button
            id="sync-move-keep"
            variant="primary"
            autoFocus
            disabled={isMoving}
            onClick={onKeep}
          >
            {t("sync.move_confirm_keep")}
          </Button>
        </>
      }
    >
      <p className="text-m3-on-surface-variant leading-relaxed break-words">
        {t("sync.move_confirm_message", { folder })}
      </p>
    </Dialog>
  );
}
