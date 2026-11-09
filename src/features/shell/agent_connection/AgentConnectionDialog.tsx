import { Trans, useTranslation } from "react-i18next";
import type { AgentConnectionRequest, ProfileList } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { formatIsoTime } from "@/ui/format/date";
import type { I18nMessage } from "@/ui/format/i18n";
import { useActiveAfterAMoment } from "./useActiveAfterAMoment";
import { useKeepFocusOnActions } from "./useKeepFocusOnActions";

interface AgentConnectionDialogProps {
  request: AgentConnectionRequest;
  /** The user whose program asks; null when the system does not say. */
  user: string | null;
  /** The profiles and the one in use (AGT-100); null until read. */
  profiles: ProfileList | null;
  /** Why the profiles could not be read, if they could not (F27). */
  profilesError?: I18nMessage | null;
  onAnswer: (requestId: number, allow: boolean) => void;
}

const DIALOG_ID = "agent-connection-dialog";

/**
 * AGT-032 — an agent asks to connect; the dialog is the validated mock of #770. It names
 * the agent and says what a yes covers in one paragraph: the profile in use, which the
 * session works in (AGT-100), and with several, that the others are out of its reach
 * (AGT-101). Refusing is the filled action, and the dialog closes only by an answer:
 * neither its corner nor its backdrop dismisses it. It is hard to approve blindly: "Allow"
 * is inactive for a moment after the dialog appears, so a click aimed at what was there
 * before allows nothing, and the keyboard's focus stays on its two actions.
 */
export function AgentConnectionDialog({
  request,
  user,
  profiles,
  profilesError = null,
  onAnswer,
}: AgentConnectionDialogProps) {
  const { t, i18n } = useTranslation();
  const canAllow = useActiveAfterAMoment();

  useKeepFocusOnActions(DIALOG_ID);

  const inUse = profiles?.profiles.find((summary) => summary.profile.id === profiles.in_use);
  const severalProfiles = (profiles?.profiles.length ?? 0) > 1;

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
        <p id="agent-connection-covers">
          {inUse ? (
            <Trans
              i18nKey="agent.connection_reach"
              values={{ profile: inUse.profile.name ?? t("profile.default_name") }}
              components={{ b: <strong /> }}
            />
          ) : (
            t("agent.connection_reach_unread")
          )}
          {severalProfiles && ` ${t("agent.connection_other_profiles")}`}
          {` ${t("agent.connection_asks")}`}
        </p>
        {profilesError && (
          <p id="agent-connection-profiles-error" role="alert" className="text-m3-error">
            {t(profilesError.key, profilesError.vars)}
          </p>
        )}
      </div>
    </Dialog>
  );
}
