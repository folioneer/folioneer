import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AgentConnectionRequest } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { formatIsoTime } from "@/ui/format/date";

interface AgentConnectionDialogProps {
  request: AgentConnectionRequest;
  /** The user whose program asks; null when the system does not say. */
  user: string | null;
  onAnswer: (requestId: number, allow: boolean) => void;
}

const DIALOG_ID = "agent-connection-dialog";

/** How long a dialog that just appeared keeps "Allow" inactive, in milliseconds. */
export const ALLOW_AFTER_MS = 800;

/**
 * AGT-032 — an agent asks to connect. The dialog names it and says what a yes covers;
 * refusing is the filled action, and the dialog closes only by an answer: neither its
 * corner nor its backdrop dismisses it. It is hard to approve blindly: "Allow" is
 * inactive for a moment after the dialog appears, so a click aimed at what was there
 * before allows nothing, and the keyboard's focus stays on its two actions.
 */
export function AgentConnectionDialog({ request, user, onAnswer }: AgentConnectionDialogProps) {
  const { t, i18n } = useTranslation();
  const [canAllow, setCanAllow] = useState(false);

  useEffect(() => {
    const timer = window.setTimeout(() => setCanAllow(true), ALLOW_AFTER_MS);
    return () => window.clearTimeout(timer);
  }, []);

  // Tab moves between the dialog's actions and never to what lies behind it.
  useEffect(() => {
    const keepFocus = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return;
      const actions = Array.from(
        document.querySelectorAll<HTMLButtonElement>(`#${DIALOG_ID} button:not(:disabled)`),
      );
      if (actions.length === 0) return;
      event.preventDefault();
      const current = actions.indexOf(document.activeElement as HTMLButtonElement);
      const step = event.shiftKey ? -1 : 1;
      actions[(current + step + actions.length) % actions.length]?.focus();
    };
    document.addEventListener("keydown", keepFocus, true);
    return () => document.removeEventListener("keydown", keepFocus, true);
  }, []);

  const rows: [string, string][] = [
    [t("agent.connection_agent"), request.client],
    [
      t("agent.connection_started_by"),
      user
        ? t("agent.connection_started_by_user", { user })
        : t("agent.connection_started_by_unknown"),
    ],
    [t("agent.connection_asked_at"), formatIsoTime(request.asked_at, i18n.language)],
  ];

  const actions = (
    <>
      <Button
        id="agent-connection-allow"
        variant="outline"
        disabled={!canAllow}
        onClick={() => onAnswer(request.id, true)}
      >
        {t("agent.connection_allow")}
      </Button>
      <Button
        id="agent-connection-refuse"
        variant="primary"
        autoFocus
        onClick={() => onAnswer(request.id, false)}
      >
        {t("agent.connection_refuse")}
      </Button>
    </>
  );

  return (
    <Dialog
      id={DIALOG_ID}
      isOpen
      onClose={() => onAnswer(request.id, false)}
      title={t("agent.connection_title")}
      actions={actions}
      maxWidth="max-w-xl"
      disableClose
    >
      <div className="flex flex-col gap-4 text-sm text-m3-on-surface">
        <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 rounded-2xl bg-m3-surface-variant/60 px-4 py-3">
          {rows.map(([label, value]) => (
            <div key={label} className="contents">
              <dt className="text-m3-on-surface-variant">{label}</dt>
              <dd className="font-medium break-words">{value}</dd>
            </div>
          ))}
        </dl>
        <div>
          <p>{t("agent.connection_covers")}</p>
          <ul className="list-disc pl-5">
            <li>{t("agent.connection_covers_read")}</li>
          </ul>
        </div>
        <p>{t("agent.connection_never")}</p>
      </div>
    </Dialog>
  );
}
