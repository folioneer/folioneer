import { AgentConnectionDialog } from "./AgentConnectionDialog";
import { useAgentConnections } from "./useAgentConnections";

/**
 * Shell-level mount of the connection dialog (AGT-032): shown over whatever view is open
 * while an agent waits for the owner's answer.
 */
export function AgentConnectionMount() {
  const { request, user, answer } = useAgentConnections();

  if (!request) return null;

  return <AgentConnectionDialog key={request.id} request={request} user={user} onAnswer={answer} />;
}
