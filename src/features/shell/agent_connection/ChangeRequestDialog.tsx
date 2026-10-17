import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ChangeRequest, TransactionShown } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { ALLOW_AFTER_MS } from "./AgentConnectionDialog";
import { movedRow, transactionRows } from "./transactionRows";
import { useKeepFocusOnActions } from "./useKeepFocusOnActions";

interface ChangeRequestDialogProps {
  request: ChangeRequest;
  onAnswer: (requestId: number, change: boolean) => void;
}

const DIALOG_ID = "agent-change-dialog";

/**
 * AGT-089 — an agent asks to change a transaction the owner typed. The dialog shows it as it
 * is and as it would become, a value that changes struck through in the first column and in
 * the accent colour in the second, and covers that one change only. Refusing is the filled
 * action in focus; "Change this transaction" is inactive for a moment after the dialog
 * appears; the keyboard's focus stays on the two actions, and the dialog closes only by an
 * answer.
 */
export function ChangeRequestDialog({ request, onAnswer }: ChangeRequestDialogProps) {
  const { t, i18n } = useTranslation();
  const [canChange, setCanChange] = useState(false);

  useEffect(() => {
    const timer = window.setTimeout(() => setCanChange(true), ALLOW_AFTER_MS);
    return () => window.clearTimeout(timer);
  }, []);

  useKeepFocusOnActions(DIALOG_ID);

  // Both states have the same rows: a correction keeps the transaction's type and asset
  // (TRX-031), so no row is in one and not the other. AGT-088 — the core names what moves.
  const moved = new Set<string>(request.moved);
  const rows = (shown: TransactionShown) => [
    ...transactionRows(shown, t, i18n.language),
    ...request.moved.flatMap((value) => movedRow(value, shown, t)),
  ];
  const was = rows(request.transaction);
  const becomes = new Map(rows(request.becomes).map(([key, , value]) => [key, value]));

  const actions = (
    <>
      <Button
        id="agent-change-change"
        variant="outline"
        disabled={!canChange}
        onClick={() => onAnswer(request.id, true)}
      >
        {t("agent.change_change")}
      </Button>
      <Button
        id="agent-change-refuse"
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
      title={t("agent.change_title", { client: request.client })}
      actions={actions}
      maxWidth="max-w-xl"
      disableClose
    >
      <div className="flex flex-col gap-4 text-sm text-m3-on-surface">
        <div className="rounded-2xl bg-m3-surface-variant/60 px-4 py-3">
          <table className="w-full table-fixed border-separate border-spacing-x-0 border-spacing-y-2 text-left">
            <thead>
              <tr>
                <td className="w-28" />
                <th scope="col" className="font-normal text-m3-on-surface-variant">
                  {t("agent.change_as_it_is")}
                </th>
                <th scope="col" className="font-normal text-m3-on-surface-variant">
                  {t("agent.change_as_it_becomes")}
                </th>
              </tr>
            </thead>
            <tbody>
              {was.map(([key, label, value]) => {
                const after = becomes.get(key) ?? value;
                const changed = moved.has(key);
                return (
                  <tr key={key}>
                    <th scope="row" className="pr-6 font-normal text-m3-on-surface-variant">
                      {label}
                    </th>
                    <td
                      id={`agent-change-was-${key}`}
                      className={
                        changed
                          ? "pr-6 text-m3-on-surface-variant line-through break-words"
                          : "pr-6 font-medium break-words"
                      }
                    >
                      {value}
                    </td>
                    <td
                      id={`agent-change-becomes-${key}`}
                      className={
                        changed
                          ? "font-semibold text-m3-primary break-words"
                          : "font-medium break-words"
                      }
                    >
                      {after}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        <div>
          <p className="font-medium">{t("agent.removal_becomes")}</p>
          <p className="text-m3-on-surface-variant">{t("agent.change_becomes_changed")}</p>
        </div>
        <p className="text-m3-on-surface-variant">{t("agent.change_covers")}</p>
      </div>
    </Dialog>
  );
}
