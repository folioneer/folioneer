import { useTranslation } from "react-i18next";
import type { ConflictNotice } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { noticeToI18n } from "../../shared/presenter";
import { useNoticeList } from "./useNoticeList";

interface NoticeListProps {
  notices: ConflictNotice[];
  onDismissed: (noticeId: string) => void;
}

/** SYN-066 — the undismissed conflict notices, each with its own dismiss action (F25). */
export function NoticeList({ notices, onDismissed }: NoticeListProps) {
  const { t } = useTranslation();
  const { handleDismiss, dismissError } = useNoticeList({ notices, onDismissed });

  return (
    <div className="flex flex-col gap-2">
      <ul className="flex flex-col gap-2">
        {notices.map((notice) => {
          const message = noticeToI18n(notice);
          return (
            <li
              key={notice.notice_id}
              id={`sync-notice-${notice.notice_id}`}
              className="flex items-start justify-between gap-3 rounded-lg bg-m3-surface-container px-3 py-2 text-sm text-m3-on-surface"
            >
              <span>{t(message.key, message.vars)}</span>
              <Button
                id={`sync-notice-dismiss-${notice.notice_id}`}
                data-testid={`sync-notice-dismiss-${notice.notice_id}`}
                variant="ghost"
                size="sm"
                onClick={() => void handleDismiss(notice.notice_id)}
              >
                {t("sync.dismiss_notice")}
              </Button>
            </li>
          );
        })}
      </ul>
      {dismissError && (
        <span className="text-xs text-m3-error">{t(dismissError.key, dismissError.vars)}</span>
      )}
    </div>
  );
}
