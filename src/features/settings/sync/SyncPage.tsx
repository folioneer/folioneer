import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { ConfirmationDialog } from "@/ui/components/modal/Dialog";
import { formatIsoDateTime } from "@/ui/format/date";
import { formatHoldingInconsistency } from "@/ui/format/holdingInconsistency";
import { messageText } from "@/ui/format/i18n";
import { syncFailureToI18n } from "../shared/presenter";
import { ChangeFolderDialog } from "./ChangeFolderDialog";
import { EnableSyncModal } from "./enable_modal/EnableSyncModal";
import { NoticeList } from "./notices/NoticeList";
import { RenameDeviceDialog } from "./RenameDeviceDialog";
import { useSyncPage } from "./useSyncPage";

const HEALTH_DOT = {
  up_to_date: "bg-m3-primary",
  needs_attention: "bg-m3-error",
  paused: "bg-m3-outline",
} as const;

/** A titled group of the page: what it holds, and an optional line saying what it is for. */
function Group({
  id,
  title,
  note,
  children,
}: {
  id: string;
  title: string;
  note?: string;
  children: React.ReactNode;
}) {
  return (
    <section
      id={id}
      className="flex flex-col gap-3 rounded-2xl border border-m3-outline-variant p-4"
    >
      <div className="flex flex-col gap-0.5">
        <h3 className="text-sm font-medium text-m3-on-surface">{title}</h3>
        {note && <p className="text-xs text-m3-on-surface-variant">{note}</p>}
      </div>
      {children}
    </section>
  );
}

/** One line of a group: what it is, what it does, and its action on the right. */
function ActionRow({
  label,
  hint,
  children,
}: {
  label: React.ReactNode;
  hint: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex min-w-0 flex-col">
        <span className="text-sm text-m3-on-surface">{label}</span>
        <span className="text-xs text-m3-on-surface-variant">{hint}</span>
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

/**
 * SYN-010/017/061/063/066/069–074/082/084 — the sync page: the honest positioning note
 * and "Enable sync" while disabled; while enabled, its health with the routine actions,
 * the computers, this computer's name and folder, and the actions that cannot be undone,
 * kept apart. Stable ids per F25.
 */
export function SyncPage() {
  const { t, i18n } = useTranslation();
  const state = useSyncPage();
  const [openDialog, setOpenDialog] = useState<"rename" | "folder" | null>(null);
  // A dialog shows only the rejection of what it submitted: the last action's error is
  // forgotten when one opens, and its own when it closes.
  const dialog = openDialog;
  const setDialog = (next: "rename" | "folder" | null) => {
    state.clearActionError();
    setOpenDialog(next);
  };

  const formatWhen = (iso: string | null) =>
    iso === null ? t("sync.last_sync_never") : formatIsoDateTime(iso, i18n.language);
  const health = state.health;

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <div className="flex max-w-3xl flex-col gap-5 p-6">
        <h2 className="text-2xl font-medium text-m3-on-surface">{t("sync.title")}</h2>

        {state.isLoading && (
          <span className="text-xs text-m3-on-surface-variant">{t("sync.loading")}</span>
        )}
        {state.loadError && (
          <span className="text-xs text-m3-error">{messageText(t, state.loadError)}</span>
        )}

        {!state.isLoading && !state.enabled && (
          <div>
            <Button
              id="sync-enable"
              data-testid="sync-enable"
              variant="primary"
              onClick={state.openEnableModal}
            >
              {t("sync.enable")}
            </Button>
          </div>
        )}

        {!state.isLoading && state.enabled && (
          <>
            <section
              id="sync-status"
              className="flex flex-wrap items-center justify-between gap-4 rounded-2xl bg-m3-surface-container p-4"
            >
              <div className="flex items-center gap-3">
                <span
                  aria-hidden="true"
                  className={`h-3 w-3 shrink-0 rounded-full ${HEALTH_DOT[health]}`}
                />
                <div className="flex flex-col">
                  <span id="sync-status-state" className="text-base font-medium text-m3-on-surface">
                    {t(`sync.health.${health}`)}
                  </span>
                  <span className="text-sm text-m3-on-surface-variant">
                    {t("sync.last_sync_label")}
                    {": "}
                    <span id="sync-status-last-sync">{formatWhen(state.lastSyncCompletedAt)}</span>
                  </span>
                </div>
              </div>
              <div className="flex gap-2">
                {state.paused ? (
                  <Button
                    id="sync-resume"
                    data-testid="sync-resume"
                    variant="tonal"
                    size="sm"
                    onClick={() => void state.handleResume()}
                  >
                    {t("sync.resume")}
                  </Button>
                ) : (
                  <Button
                    id="sync-pause"
                    data-testid="sync-pause"
                    variant="tonal"
                    size="sm"
                    onClick={() => void state.handlePause()}
                  >
                    {t("sync.pause")}
                  </Button>
                )}
                <Button
                  id="sync-now"
                  data-testid="sync-now"
                  variant="primary"
                  size="sm"
                  loading={state.isSyncing}
                  disabled={state.isSyncing}
                  onClick={() => void state.handleSyncNow()}
                >
                  {state.isSyncing ? t("sync.syncing") : t("sync.sync_now")}
                </Button>
              </div>
            </section>

            {(state.actionError !== null && dialog === null) ||
            state.failures.length > 0 ||
            state.heldBackCount > 0 ? (
              <div
                id="sync-problems"
                className="flex flex-col gap-1 rounded-2xl border border-m3-error p-4"
              >
                {state.actionError && dialog === null && (
                  <span id="sync-action-error" className="text-xs text-m3-error">
                    {t(state.actionError.key, state.actionError.vars)}
                  </span>
                )}

                {state.failures.length > 0 && (
                  <ul id="sync-failures" className="flex flex-col gap-1 text-xs text-m3-error">
                    {state.failures.map((failure, index) => {
                      const message = syncFailureToI18n(failure);
                      return (
                        <li key={message.key} data-testid={`sync-failure-${index}`}>
                          {t(message.key, message.vars)}
                        </li>
                      );
                    })}
                  </ul>
                )}

                {state.heldBackCount > 0 && (
                  <span
                    id="sync-held-back"
                    data-testid="sync-held-back"
                    className="text-xs text-m3-on-surface-variant"
                  >
                    {t("sync.held_back", {
                      count: state.heldBackCount,
                      since: formatWhen(state.oldestHeldBackSince),
                    })}
                  </span>
                )}
              </div>
            ) : null}

            <Group
              id="sync-computers"
              title={t("sync.computers_title")}
              note={t("sync.computers_note")}
            >
              <table className="w-full text-sm text-m3-on-surface">
                <caption className="sr-only">{t("sync.computers_title")}</caption>
                <thead>
                  <tr className="text-left text-xs text-m3-on-surface-variant">
                    <th scope="col" className="pb-1 font-medium">
                      {t("sync.computer_column")}
                    </th>
                    <th scope="col" className="pb-1 font-medium">
                      {t("sync.version_column")}
                    </th>
                    <th scope="col" className="pb-1 font-medium">
                      {t("sync.published_column")}
                    </th>
                    <th scope="col" className="pb-1 font-medium">
                      {t("sync.applied_column")}
                    </th>
                  </tr>
                </thead>
                <tbody id="sync-roster">
                  <tr id="sync-roster-this-computer">
                    <td className="py-1">
                      <span id="sync-status-device-name">{state.deviceName}</span>
                      <span className="ml-1 text-xs text-m3-on-surface-variant">
                        {t("sync.this_computer_suffix")}
                      </span>
                    </td>
                    <td id="sync-status-app-version">
                      {t("sync.app_version", { version: state.appVersion })}
                    </td>
                    <td
                      className="text-m3-on-surface-variant"
                      aria-label={t("sync.not_applicable")}
                    >
                      —
                    </td>
                    <td
                      className="text-m3-on-surface-variant"
                      aria-label={t("sync.not_applicable")}
                    >
                      —
                    </td>
                  </tr>
                  {state.roster.map((entry) => (
                    <tr key={entry.deviceId} id={`sync-roster-${entry.deviceId}`}>
                      <td className="py-1">{entry.deviceName}</td>
                      <td id={`sync-roster-${entry.deviceId}-version`}>
                        {entry.appVersion === null
                          ? t("sync.app_version_unknown")
                          : t("sync.app_version", { version: entry.appVersion })}
                      </td>
                      <td
                        id={`sync-roster-${entry.deviceId}-published`}
                        className={entry.publishedChanges === 0 ? "text-m3-error" : ""}
                      >
                        {entry.publishedChanges === 0
                          ? t("sync.published_nothing")
                          : t("sync.published_changes", { count: entry.publishedChanges })}
                      </td>
                      <td>
                        {entry.lastAppliedAt === null
                          ? t("sync.roster_never_applied")
                          : formatWhen(entry.lastAppliedAt)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {state.roster.length === 0 && (
                <p id="sync-roster-empty" className="text-xs text-m3-on-surface-variant">
                  {t("sync.roster_empty")}
                </p>
              )}
            </Group>

            {state.notices.length > 0 && (
              <div className="flex flex-col gap-1">
                <span className="text-xs font-medium text-m3-on-surface-variant">
                  {t("sync.notices_title")}
                </span>
                <NoticeList notices={state.notices} onDismissed={() => void state.refresh()} />
              </div>
            )}

            {state.inconsistentHoldings.length > 0 && (
              <div className="flex flex-col gap-1">
                <span className="text-xs font-medium text-m3-on-surface-variant">
                  {t("sync.inconsistent_title")}
                </span>
                <ul
                  id="sync-inconsistent-holdings"
                  className="flex flex-col gap-1 text-sm text-m3-error"
                >
                  {state.inconsistentHoldings.map((holding) => {
                    const reason = formatHoldingInconsistency(holding.reason);
                    return (
                      <li key={`${holding.account_id}-${holding.asset_id}`}>
                        {t("sync.inconsistent_row", {
                          accountName: holding.account_name,
                          assetName: holding.asset_name,
                          reason: t(reason.key, reason.vars),
                        })}
                      </li>
                    );
                  })}
                </ul>
              </div>
            )}
            <Group id="sync-this-computer" title={t("sync.device_name_label")}>
              <ActionRow label={state.deviceName} hint={t("sync.rename_hint")}>
                <Button
                  id="sync-rename"
                  data-testid="sync-rename"
                  variant="outline"
                  size="sm"
                  onClick={() => setDialog("rename")}
                >
                  {t("sync.rename_action")}
                </Button>
              </ActionRow>
              <ActionRow
                label={
                  <span id="sync-status-folder" className="break-all">
                    {state.folder}
                  </span>
                }
                hint={t("sync.folder_hint")}
              >
                <Button
                  id="sync-change-folder"
                  data-testid="sync-change-folder"
                  variant="outline"
                  size="sm"
                  onClick={() => setDialog("folder")}
                >
                  {t("sync.change_folder")}
                </Button>
              </ActionRow>
            </Group>

            <Group
              id="sync-danger-zone"
              title={t("sync.danger_title")}
              note={t("sync.danger_note")}
            >
              <ActionRow label={t("sync.leave")} hint={t("sync.leave_hint")}>
                <Button
                  id="sync-leave"
                  data-testid="sync-leave"
                  variant="outline"
                  size="sm"
                  onClick={state.requestLeave}
                >
                  {t("sync.leave")}
                </Button>
              </ActionRow>
              <ActionRow label={t("sync.start_over")} hint={t("sync.start_over_hint")}>
                <Button
                  id="sync-start-over"
                  data-testid="sync-start-over"
                  variant="danger"
                  size="sm"
                  onClick={state.openStartOverModal}
                >
                  {t("sync.start_over")}
                </Button>
              </ActionRow>
            </Group>
          </>
        )}

        <p className="text-xs text-m3-on-surface-variant">{t("sync.local_copy_note")}</p>

        <EnableSyncModal
          isOpen={state.isEnableModalOpen}
          onClose={state.closeEnableModal}
          onSuccess={() => {
            state.closeEnableModal();
            void state.refresh();
          }}
          variant="enable"
        />
        <EnableSyncModal
          isOpen={state.isStartOverModalOpen}
          onClose={state.closeStartOverModal}
          onSuccess={() => {
            state.closeStartOverModal();
            void state.refresh();
          }}
          variant="start-over"
        />

        <ConfirmationDialog
          isOpen={state.confirmingLeave}
          onCancel={state.cancelLeave}
          onConfirm={() => void state.confirmLeave()}
          title={t("sync.leave_confirm_title")}
          message={t("sync.leave_confirm_message")}
          confirmLabel={t("sync.leave")}
          cancelLabel={t("action.cancel")}
          variant="danger"
          confirmId="sync-leave-confirm"
        />

        {dialog === "rename" && (
          <RenameDeviceDialog
            currentName={state.deviceName ?? ""}
            error={messageText(t, state.actionError)}
            onSubmit={state.handleRename}
            onClose={() => setDialog(null)}
          />
        )}
        {dialog === "folder" && (
          <ChangeFolderDialog
            currentFolder={state.folder ?? ""}
            error={messageText(t, state.actionError)}
            onSubmit={state.handleChangeFolder}
            onBrowse={state.handleBrowseFolder}
            onClose={() => setDialog(null)}
          />
        )}
      </div>
    </div>
  );
}
