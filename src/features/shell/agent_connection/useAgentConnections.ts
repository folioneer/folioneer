import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  AgentConnectionRequest,
  AgentSession,
  ChangeRequest,
  RemovalRequest,
} from "@/bindings";
import { logger } from "@/lib/logger";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import {
  answerAgentConnection,
  answerChangeRequest,
  answerRemovalRequest,
  disconnectAgent,
  getAgentConnectionState,
  onAgentConnectionChanged,
  removeAgentRecordings,
} from "../gateway";

export interface UseAgentConnectionsResult {
  /** The agent waiting for the owner's answer: the oldest one, others wait their turn. */
  request: AgentConnectionRequest | null;
  /**
   * AGT-083 — the removal request to answer: the oldest request of either kind, once no
   * agent waits to connect (AGT-088).
   */
  removal: RemovalRequest | null;
  /** AGT-089 — the change request to answer, on the same terms. */
  change: ChangeRequest | null;
  /** The agents connected, oldest first. */
  sessions: AgentSession[];
  /** The user whose programs can connect; null when the system does not say. */
  user: string | null;
  answer: (requestId: number, allow: boolean) => void;
  answerRemoval: (requestId: number, remove: boolean) => void;
  answerChange: (requestId: number, change: boolean) => void;
  disconnect: (sessionId: number) => void;
  /** AGT-053 — removes everything the session recorded, and says how much went. */
  removeRecordings: (sessionId: number) => void;
}

/**
 * AGT-081 / AGT-088 — the sentence for a refused command; a removal or a change that did not
 * happen says why.
 */
function refusalKey(error: { code: string; reason?: string }): string {
  const ask = error.code === "TransactionNotChanged" ? "change" : "removal";
  if (error.reason === "TransactionChanged") return `agent.${ask}_changed`;
  if (error.reason === "TransactionNotFound") return `agent.${ask}_gone`;
  return `error.${error.code}`;
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
  const [removals, setRemovals] = useState<RemovalRequest[]>([]);
  const [changes, setChanges] = useState<ChangeRequest[]>([]);
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
      setRemovals(state.removals);
      setChanges(state.changes);
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
  // state is read again and the reason is said. AGT-081 — a removal that did not happen
  // says why, by the reason's code.
  const settle = useCallback(
    (
      call: Promise<
        { status: "ok" } | { status: "error"; error: { code: string; reason?: string } }
      >,
    ) => {
      call
        .then((result) => {
          if (result.status === "error") {
            showSnackbar(t(refusalKey(result.error)), "error");
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
  const answerRemoval = useCallback(
    (requestId: number, remove: boolean) => settle(answerRemovalRequest(requestId, remove)),
    [settle],
  );
  const answerChange = useCallback(
    (requestId: number, change: boolean) => settle(answerChangeRequest(requestId, change)),
    [settle],
  );
  const disconnect = useCallback(
    (sessionId: number) => settle(disconnectAgent(sessionId)),
    [settle],
  );

  const removeRecordings = useCallback(
    (sessionId: number) => {
      removeAgentRecordings(sessionId)
        .then((result) => {
          if (result.status === "error") {
            showSnackbar(t(`error.${result.error.code}`), "error");
            return;
          }
          const { removed, kept } = result.data;
          const went = t("agent.session_removed", { count: removed });
          showSnackbar(
            kept > 0 ? `${went} ${t("agent.session_kept", { count: kept })}` : went,
            kept > 0 ? "error" : "success",
          );
        })
        .catch((e) => {
          logger.error("[useAgentConnections] removal failed", { error: e });
          showSnackbar(t("error.Unknown"), "error");
        })
        .finally(() => void refresh());
    },
    [refresh, showSnackbar, t],
  );

  const request = requests[0] ?? null;
  // AGT-088 — the two kinds share one order, the core's: the lower id asked first.
  const removal = removals[0] ?? null;
  const change = changes[0] ?? null;
  const removalFirst = removal !== null && (change === null || removal.id < change.id);
  return {
    request,
    removal: request || !removalFirst ? null : removal,
    change: request || removalFirst ? null : change,
    sessions,
    user,
    answer,
    answerRemoval,
    answerChange,
    disconnect,
    removeRecordings,
  };
}
