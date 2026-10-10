import { Bot } from "lucide-react";
import { useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { AgentSessionDialog } from "./AgentSessionDialog";
import { useAgentConnections } from "./useAgentConnections";

// White on the header's fixed indigo gradient, outlined so both read as controls.
const ON_HEADER = "border-white/60 text-white hover:enabled:bg-white/15";

/**
 * AGT-033 / AGT-034 / AGT-052 — the header says which agent is connected, opens what its
 * session did and offers to disconnect it. Nothing shows while no agent is connected.
 */
export function AgentIndicator() {
  const { t } = useTranslation();
  const { request, sessions, disconnect, removeRecordings } = useAgentConnections();
  const [shown, setShown] = useState<number | null>(null);
  // A session that ended closes its dialog: there is nothing left to show.
  const shownSession = sessions.find((session) => session.id === shown) ?? null;

  if (sessions.length === 0) return null;

  return (
    <div id="agent-indicator" className="flex items-center gap-2">
      {sessions.map((session) => (
        <div key={session.id} className="flex items-center gap-2">
          <button
            type="button"
            id={`agent-connected-${session.id}`}
            title={session.client}
            aria-label={t("agent.session_open", { client: session.client })}
            className="inline-flex items-center gap-2 h-8 px-3 max-w-64 rounded-xl border border-white/60 text-xs font-medium cursor-pointer hover:bg-white/15"
            onClick={() => setShown(session.id)}
          >
            <Bot size={14} aria-hidden="true" className="shrink-0" />
            <span className="truncate">{t("agent.connected", { client: session.client })}</span>
          </button>
          <Button
            id={`agent-disconnect-${session.id}`}
            variant="outline"
            size="sm"
            className={ON_HEADER}
            aria-label={t("agent.disconnect_client", { client: session.client })}
            onClick={() => disconnect(session.id)}
          >
            {t("agent.disconnect")}
          </Button>
        </div>
      ))}
      {/* Out of the header, which stacks under the views: the dialog covers the window. An
          agent that asks to connect comes first (AGT-032): the dialog gives way to it. */}
      {shownSession &&
        request === null &&
        createPortal(
          <AgentSessionDialog
            session={shownSession}
            onClose={() => setShown(null)}
            onDisconnect={disconnect}
            onRemoveRecordings={removeRecordings}
          />,
          document.body,
        )}
    </div>
  );
}
