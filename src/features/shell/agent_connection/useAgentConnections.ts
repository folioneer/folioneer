import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AgentConnectionRequest, AgentSession } from "@/bindings";
import { logger } from "@/lib/logger";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import {
  answerAgentConnection,
  disconnectAgent,
  getAgentConnectionState,
  onAgentConnectionChanged,
} from "../gateway";

export interface UseAgentConnectionsResult {
  /** The agent waiting for the owner's answer: the oldest one, others wait their turn. */
  request: AgentConnectionRequest | null;
  /** The agents connected, oldest first. */
  sessions: AgentSession[];
  /** The user whose programs can connect; null when the system does not say. */
  user: string | null;
  answer: (requestId: number, allow: boolean) => void;
  disconnect: (sessionId: number) => void;
}

/**
 * AGT-036 — who asks to connect and who is connected: read on mount and again after
 * every `AgentConnectionChanged`, so the window always shows what the core holds.
 */
export function useAgentConnections(): UseAgentConnectionsResult {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const [requests, setRequests] = useState<AgentConnectionRequest[]>([]);
  const [sessions, setSessions] = useState<AgentSession[]>([]);
  const [user, setUser] = useState<string | null>(null);

  // Reads may answer out of order: only the latest one asked is shown.
  const latestRead = useRef(0);

  const refresh = useCallback(async () => {
    latestRead.current += 1;
    const read = latestRead.current;
    try {
      const state = await getAgentConnectionState();
      if (read !== latestRead.current) return;
      setRequests(state.requests);
      setSessions(state.sessions);
      setUser(state.user);
    } catch (e) {
      logger.error("[useAgentConnections] state not read", { error: e });
    }
  }, []);

  useEffect(() => {
    void refresh();
    const unlistenPromise = onAgentConnectionChanged(() => void refresh());
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refresh]);

  // AGT-039 — a refused command means the request or the session is already gone: the
  // state is read again and the reason is said.
  const settle = useCallback(
    (call: Promise<{ status: "ok" } | { status: "error"; error: { code: string } }>) => {
      call
        .then((result) => {
          if (result.status === "error") {
            showSnackbar(t(`error.${result.error.code}`), "error");
          }
        })
        .catch((e) => {
          logger.error("[useAgentConnections] command failed", { error: e });
          showSnackbar(t("error.Unknown"), "error");
        })
        .finally(() => void refresh());
    },
    [refresh, showSnackbar, t],
  );

  const answer = useCallback(
    (requestId: number, allow: boolean) => settle(answerAgentConnection(requestId, allow)),
    [settle],
  );
  const disconnect = useCallback(
    (sessionId: number) => settle(disconnectAgent(sessionId)),
    [settle],
  );

  return { request: requests[0] ?? null, sessions, user, answer, disconnect };
}
