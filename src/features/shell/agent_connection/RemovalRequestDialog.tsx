import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { RemovalRequest } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { ALLOW_AFTER_MS } from "./AgentConnectionDialog";
import { transactionRows } from "./transactionRows";
import { useKeepFocusOnActions } from "./useKeepFocusOnActions";

interface RemovalRequestDialogProps {
  request: RemovalRequest;
  onAnswer: (requestId: number, remove: boolean) => void;
}

const DIALOG_ID = "agent-removal-dialog";

/**
 * AGT-083 — an agent asks to remove a transaction the owner typed. The dialog shows the
 * transaction in full and what would become of it, and covers that one transaction only.
 * Refusing is the filled action in focus; "Remove this transaction" is inactive for a
 * moment after the dialog appears; the keyboard's focus stays on the two actions, and the
 * dialog closes only by an answer.
 */
export function RemovalRequestDialog({ request, onAnswer }: RemovalRequestDialogProps) {
  const { t, i18n } = useTranslation();
  const [canRemove, setCanRemove] = useState(false);

  useEffect(() => {
    const timer = window.setTimeout(() => setCanRemove(true), ALLOW_AFTER_MS);
    return () => window.clearTimeout(timer);
  }, []);

  useKeepFocusOnActions(DIALOG_ID);

  const rows = transactionRows(request.transaction, t, i18n.language);

  const actions = (
    <>
      <Button
        id="agent-removal-remove"
        variant="outline"
        disabled={!canRemove}
        onClick={() => onAnswer(request.id, true)}
      >
        {t("agent.removal_remove")}
      </Button>
      <Button
        id="agent-removal-refuse"
        variant="primary"
        autoFocus
        onClick={() => onAnswer(request.id, false)}
      >
        {t("agent.removal_refuse")}
      </Button>
    </>
  );

  return (
    <Dialog
      id={DIALOG_ID}
      isOpen
      onClose={() => onAnswer(request.id, false)}
      title={t("agent.removal_title", { client: request.client })}
      actions={actions}
      maxWidth="max-w-xl"
      disableClose
    >
      <div className="flex flex-col gap-4 text-sm text-m3-on-surface">
        <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 rounded-2xl bg-m3-surface-variant/60 px-4 py-3">
          {rows.map(([key, label, value]) => (
            <div key={key} className="contents">
              <dt className="text-m3-on-surface-variant">{label}</dt>
              <dd className="font-medium break-words">{value}</dd>
            </div>
          ))}
        </dl>
        <div>
          <p className="font-medium">{t("agent.removal_becomes")}</p>
          <p className="text-m3-on-surface-variant">{t("agent.removal_becomes_removed")}</p>
        </div>
        <p className="text-m3-on-surface-variant">{t("agent.removal_covers")}</p>
      </div>
    </Dialog>
  );
}
