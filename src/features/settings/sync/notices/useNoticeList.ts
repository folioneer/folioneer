import { useCallback, useState } from "react";
import type { ConflictNotice } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";
import { dismissConflictNotice } from "../../gateway";
import { syncErrorToI18n } from "../../shared/presenter";

export interface UseNoticeListOptions {
  notices: ConflictNotice[];
  /** Called with the dismissed notice's id; the owner re-reads the status. */
  onDismissed: (noticeId: string) => void;
}

export interface UseNoticeListResult {
  handleDismiss: (noticeId: string) => Promise<void>;
  dismissError: I18nMessage | null;
}

/** SYN-066 — dismisses conflict notices one at a time. */
export function useNoticeList({ onDismissed }: UseNoticeListOptions): UseNoticeListResult {
  const [dismissError, setDismissError] = useState<I18nMessage | null>(null);

  const handleDismiss = useCallback(
    async (noticeId: string) => {
      setDismissError(null);
      const result = await dismissConflictNotice(noticeId);
      if (result.status === "ok") {
        onDismissed(noticeId);
      } else {
        setDismissError(syncErrorToI18n(result.error));
      }
    },
    [onDismissed],
  );

  return { handleDismiss, dismissError };
}
