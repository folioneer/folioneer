import { AgentConnectionDialog } from "./AgentConnectionDialog";
import { RemovalRequestDialog } from "./RemovalRequestDialog";
import { useAgentConnections } from "./useAgentConnections";
import { useAgentRecordingNotice } from "./useAgentRecordingNotice";

/**
 * Shell-level mount of the agent's asks, shown over whatever view is open: an agent waiting
 * to connect (AGT-032) first, then an agent asking to remove a transaction the owner typed
 * (AGT-083), one at a time, oldest first — and what an agent records, said on every view
 * (AGT-084).
 */
export function AgentConnectionMount() {
  const { request, removal, sessions, user, answer, answerRemoval } = useAgentConnections();
  useAgentRecordingNotice(sessions);

  if (request) {
    return (
      <AgentConnectionDialog key={request.id} request={request} user={user} onAnswer={answer} />
    );
  }
  if (removal) {
    return <RemovalRequestDialog key={removal.id} request={removal} onAnswer={answerRemoval} />;
  }
  return null;
}
