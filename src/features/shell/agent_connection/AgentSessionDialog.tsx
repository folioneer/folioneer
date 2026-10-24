import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { AgentSession } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { formatIsoDateNumeric, formatIsoTime } from "@/ui/format/date";

interface AgentSessionDialogProps {
  session: AgentSession;
  onClose: () => void;
  onDisconnect: (sessionId: number) => void;
  onRemoveRecordings: (sessionId: number) => void;
}

/**
 * AGT-054 — the session dialog: what a connected agent did in its session (AGT-052,
 * AGT-064) and its three actions — removing everything it recorded and nothing the owner
 * typed (AGT-053), closing, disconnecting. The removal asks to confirm, and keeping is the
 * filled action in focus.
 */
export function AgentSessionDialog({
  session,
  onClose,
  onDisconnect,
  onRemoveRecordings,
}: AgentSessionDialogProps) {
  const { t, i18n } = useTranslation();
  const [confirming, setConfirming] = useState(false);
  const last = session.last_recording;
  const lastAsset = session.last_asset;

  const rows: [string, string][] = [
    [t("agent.session_since"), formatIsoTime(session.started_at, i18n.language)],
    [t("agent.session_read"), t("agent.session_read_count", { count: session.reads })],
    [
      t("agent.session_accounts"),
      session.last_account
        ? t("agent.session_accounts_count", {
            count: session.accounts_added,
            name: session.last_account,
          })
        : t("agent.session_none"),
    ],
    [
      t("agent.session_assets"),
      lastAsset
        ? t("agent.session_assets_count", {
            count: session.assets_added,
            name: lastAsset.name,
            reference: lastAsset.reference,
          })
        : t("agent.session_none"),
    ],
    [t("agent.session_recorded"), t("agent.session_recorded_count", { count: session.recordings })],
    [
      t("agent.session_last"),
      last
        ? `${t(`transaction.type_${last.kind.toLowerCase()}`)} · ${last.asset} · ${formatIsoDateNumeric(last.date, i18n.language)}`
        : t("agent.session_none"),
    ],
  ];

  const actions = (
    <>
      <Button
        id="agent-session-remove"
        variant="danger"
        className="mr-auto"
        disabled={session.recordings === 0}
        onClick={() => setConfirming(true)}
      >
        {t("agent.session_remove")}
      </Button>
      <Button id="agent-session-close" variant="secondary" onClick={onClose}>
        {t("action.close")}
      </Button>
      <Button
        id="agent-session-disconnect"
        variant="primary"
        onClick={() => onDisconnect(session.id)}
      >
        {t("agent.disconnect")}
      </Button>
    </>
  );

  const confirmation = (
    <>
      <Button
        id="agent-session-remove-confirm"
        variant="outline"
        onClick={() => {
          setConfirming(false);
          onRemoveRecordings(session.id);
        }}
      >
        {t("agent.session_confirm_remove")}
      </Button>
      <Button
        id="agent-session-remove-keep"
        variant="primary"
        autoFocus
        onClick={() => setConfirming(false)}
      >
        {t("agent.session_confirm_keep")}
      </Button>
    </>
  );

  return (
    <>
      <Dialog
        id="agent-session-dialog"
        isOpen={!confirming}
        onClose={onClose}
        title={t("agent.connected", { client: session.client })}
        actions={actions}
        maxWidth="max-w-xl"
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
          <p id="agent-session-note">
            {session.recordings > 0
              ? t("agent.session_remove_note", { count: session.recordings })
              : t("agent.session_nothing")}
          </p>
        </div>
      </Dialog>
      <Dialog
        id="agent-session-remove-dialog"
        isOpen={confirming}
        onClose={() => setConfirming(false)}
        title={t("agent.session_confirm_title", { client: session.client })}
        actions={confirmation}
      >
        <p className="text-m3-on-surface-variant leading-relaxed">
          {t("agent.session_confirm_message", { count: session.recordings })}
        </p>
      </Dialog>
    </>
  );
}
