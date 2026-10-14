import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { AgentSession } from "@/bindings";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { formatIsoDateNumeric } from "@/ui/format/date";

/** What a session had done when the window last read it. */
interface Seen {
  recorded: number;
  assetsAdded: number;
}

/**
 * AGT-084 — the window says, on every view, what an agent records and adds as it happens:
 * a session whose count of transactions recorded or of assets added — counts that only
 * rise — went up since the last read is announced, with the last of them, one message
 * replacing the one before. A correction or a cancelling moves no count, so it is not
 * announced as new; a session first read is only remembered.
 */
export function useAgentRecordingNotice(sessions: AgentSession[]): void {
  const { t, i18n } = useTranslation();
  const showSnackbar = useSnackbar();
  const seen = useRef(new Map<number, Seen>());

  useEffect(() => {
    const next = new Map<number, Seen>();
    for (const session of sessions) {
      const before = seen.current.get(session.id);
      const now = { recorded: session.transactions_recorded, assetsAdded: session.assets_added };
      next.set(session.id, now);
      if (!before) continue;
      const last = session.last_recording;
      if (now.recorded > before.recorded && last) {
        showSnackbar(
          t("agent.notice_recorded", {
            agent: session.client,
            kind: t(`transaction.type_${last.kind.toLowerCase()}`),
            asset: last.asset,
            date: formatIsoDateNumeric(last.date, i18n.language),
          }),
          "info",
        );
      }
      const asset = session.last_asset;
      if (now.assetsAdded > before.assetsAdded && asset) {
        showSnackbar(
          t("agent.notice_added", {
            agent: session.client,
            name: asset.name,
            reference: asset.reference,
          }),
          "info",
        );
      }
    }
    seen.current = next;
  }, [sessions, showSnackbar, t, i18n.language]);
}
