import { Bot } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { useAgentConnections } from "./useAgentConnections";

// White on the header's fixed indigo gradient, outlined so both read as controls.
const ON_HEADER = "border-white/60 text-white hover:enabled:bg-white/15";

/**
 * AGT-033 / AGT-034 — the header says which agent is connected and offers to disconnect
 * it. Nothing shows while no agent is connected.
 */
export function AgentIndicator() {
  const { t } = useTranslation();
  const { sessions, disconnect } = useAgentConnections();

  if (sessions.length === 0) return null;

  return (
    <div id="agent-indicator" className="flex items-center gap-2">
      {sessions.map((session) => (
        <div key={session.id} className="flex items-center gap-2">
          <span
            id={`agent-connected-${session.id}`}
            title={session.client}
            className="inline-flex items-center gap-2 h-8 px-3 max-w-64 rounded-xl border border-white/60 text-xs font-medium"
          >
            <Bot size={14} aria-hidden="true" className="shrink-0" />
            <span className="truncate">{t("agent.connected", { client: session.client })}</span>
          </span>
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
    </div>
  );
}
