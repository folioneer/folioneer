import { useTranslation } from "react-i18next";
import type { ReplacementRequest } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { replacedRow, transferRows } from "./replacementRows";
import { useActiveAfterAMoment } from "./useActiveAfterAMoment";
import { useKeepFocusOnActions } from "./useKeepFocusOnActions";

interface ReplacementRequestDialogProps {
  request: ReplacementRequest;
  onAnswer: (requestId: number, replace: boolean) => void;
}

const DIALOG_ID = "agent-replacement-dialog";

/** The columns of a removed transaction's line, named for a screen reader. */
const COLUMNS = [
  "agent.transaction_account",
  "agent.transaction_date",
  "agent.transaction_kind",
  "agent.transaction_asset",
  "agent.transaction_quantity",
  "agent.transaction_amount",
] as const;

/**
 * AGT-092 — an agent asks to replace four transactions the owner typed with one transfer.
 * The dialog shows the four struck through, the transfer recorded in their place in the
 * accent colour, and what would become of it, and covers those four only. Refusing is the
 * filled action in focus; "Replace with the transfer" is inactive for a moment after the
 * dialog appears; the keyboard's focus stays on the two actions, and the dialog closes only
 * by an answer.
 */
export function ReplacementRequestDialog({ request, onAnswer }: ReplacementRequestDialogProps) {
  const { t, i18n } = useTranslation();
  const canReplace = useActiveAfterAMoment();

  useKeepFocusOnActions(DIALOG_ID);

  const { transfer } = request;
  const { rows, feeInKind } = transferRows(transfer, t, i18n.language);

  const actions = (
    <>
      <Button
        id="agent-replacement-replace"
        variant="outline"
        disabled={!canReplace}
        onClick={() => onAnswer(request.id, true)}
      >
        {t("agent.replacement_replace")}
      </Button>
      <Button
        id="agent-replacement-refuse"
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
      title={t("agent.replacement_title", { client: request.client })}
      actions={actions}
      maxWidth="max-w-2xl"
      disableClose
    >
      <div className="flex flex-col gap-4 text-sm text-m3-on-surface">
        <div className="rounded-2xl bg-m3-surface-variant/60 px-4 py-3">
          <p className="mb-2 text-m3-on-surface-variant">{t("agent.replacement_removed")}</p>
          <table className="w-full border-separate border-spacing-y-1 text-left">
            <thead className="sr-only">
              <tr>
                {COLUMNS.map((column) => (
                  <th key={column} scope="col">
                    {t(column)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {request.transactions.map((shown) => (
                <tr
                  key={shown.id}
                  id={`agent-replacement-removed-${shown.kind.toLowerCase()}`}
                  className="text-m3-on-surface-variant line-through"
                >
                  {replacedRow(shown, t, i18n.language).map(([key, value]) => (
                    <td
                      key={key}
                      className={
                        key === "quantity" || key === "amount"
                          ? "pr-4 text-right whitespace-nowrap last:pr-0"
                          : "pr-4 break-words"
                      }
                    >
                      {value}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="rounded-2xl bg-m3-surface-variant/60 px-4 py-3">
          <p className="mb-2 text-m3-on-surface-variant">{t("agent.replacement_recorded")}</p>
          <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2">
            {rows.map(([key, label, value]) => (
              <div key={key} className="contents">
                <dt className="text-m3-on-surface-variant">{label}</dt>
                <dd
                  id={`agent-replacement-transfer-${key}`}
                  className="font-semibold text-m3-primary break-words"
                >
                  {value}
                </dd>
              </div>
            ))}
          </dl>
        </div>
        <div>
          <p className="font-medium">{t("agent.removal_becomes")}</p>
          <p className="text-m3-on-surface-variant">
            {t("agent.replacement_becomes", {
              asset: transfer.asset_name,
              to: transfer.to_account,
            })}
          </p>
          {feeInKind !== null && (
            <p id="agent-replacement-fee-in-kind" className="text-m3-on-surface-variant">
              {t("agent.replacement_fee_in_kind", {
                difference: feeInKind,
                from: transfer.from_account,
              })}
            </p>
          )}
        </div>
        <p className="text-m3-on-surface-variant">{t("agent.replacement_covers")}</p>
      </div>
    </Dialog>
  );
}
