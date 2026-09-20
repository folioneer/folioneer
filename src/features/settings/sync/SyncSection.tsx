import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { TextField } from "@/ui/components/field/TextField";
import { ConfirmationDialog, Dialog } from "@/ui/components/modal/Dialog";
import { formatIsoDateTime } from "@/ui/format/date";
import { formatHoldingInconsistency } from "@/ui/format/holdingInconsistency";
import { syncFailureToI18n } from "../shared/presenter";
import { EnableSyncModal } from "./enable_modal/EnableSyncModal";
import { NoticeList } from "./notices/NoticeList";
import { useSyncSection } from "./useSyncSection";

/** The two single-field prompts of the enabled state (SYN-072 rename, SYN-074 change folder). */
type Prompt = { kind: "rename" | "folder"; value: string } | null;

/**
 * SYN-010/017/061/063/066/069–074/082/084 — Settings section: the honest
 * positioning note and "Enable sync" while disabled; the status block with
 * every device-side action while enabled. Stable ids per F25.
 */
export function SyncSection() {
  const { t, i18n } = useTranslation();
  const state = useSyncSection();
  const [prompt, setPrompt] = useState<Prompt>(null);
  const [isSubmittingPrompt, setIsSubmittingPrompt] = useState(false);

  const formatWhen = (iso: string | null) =>
    iso === null ? t("sync.last_sync_never") : formatIsoDateTime(iso, i18n.language);

  const browseForFolder = async () => {
    const picked = await state.handleBrowseFolder();
    if (picked !== null) {
      setPrompt((current) => (current ? { ...current, value: picked } : current));
    }
  };

  const promptField = (
    <TextField
      id="sync-prompt-value"
      label={
        prompt?.kind === "rename"
          ? t("sync.rename_prompt_label")
          : t("sync.change_folder_prompt_label")
      }
      value={prompt?.value ?? ""}
      error={state.actionError ? t(state.actionError.key, state.actionError.vars) : undefined}
      onChange={(event) =>
        setPrompt((current) => (current ? { ...current, value: event.target.value } : current))
      }
    />
  );

  const submitPrompt = async () => {
    if (prompt === null) return;
    setIsSubmittingPrompt(true);
    const accepted =
      prompt.kind === "rename"
        ? await state.handleRename(prompt.value)
        : await state.handleChangeFolder(prompt.value);
    setIsSubmittingPrompt(false);
    if (accepted) {
      setPrompt(null);
    }
  };

  return (
    <section className="flex flex-col gap-3">
      <span className="text-sm font-medium text-m3-on-surface-variant">{t("sync.title")}</span>
      <p className="text-xs text-m3-on-surface-variant">{t("sync.local_copy_note")}</p>

      {state.isLoading && (
        <span className="text-xs text-m3-on-surface-variant">{t("sync.loading")}</span>
      )}
      {state.loadError && (
        <span className="text-xs text-m3-error">
          {t(state.loadError.key, state.loadError.vars)}
        </span>
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
        <div className="flex flex-col gap-3">
          <dl
            id="sync-status"
            className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm text-m3-on-surface"
          >
            <dt className="text-m3-on-surface-variant">{t("sync.title")}</dt>
            <dd id="sync-status-state">
              {state.paused ? t("sync.status_paused") : t("sync.status_enabled")}
            </dd>
            <dt className="text-m3-on-surface-variant">{t("sync.device_name_label")}</dt>
            <dd>
              <span id="sync-status-device-name">{state.deviceName}</span>
              <span
                id="sync-status-app-version"
                className="ml-2 text-xs text-m3-on-surface-variant"
              >
                {t("sync.app_version", { version: state.appVersion })}
              </span>
            </dd>
            <dt className="text-m3-on-surface-variant">{t("sync.folder_label")}</dt>
            <dd id="sync-status-folder" className="break-all">
              {state.folder}
            </dd>
            <dt className="text-m3-on-surface-variant">{t("sync.last_sync_label")}</dt>
            <dd id="sync-status-last-sync">{formatWhen(state.lastSyncCompletedAt)}</dd>
          </dl>

          <div className="flex flex-wrap gap-2">
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
              id="sync-rename"
              data-testid="sync-rename"
              variant="outline"
              size="sm"
              onClick={() => setPrompt({ kind: "rename", value: state.deviceName ?? "" })}
            >
              {t("sync.rename")}
            </Button>
            <Button
              id="sync-change-folder"
              data-testid="sync-change-folder"
              variant="outline"
              size="sm"
              onClick={() => setPrompt({ kind: "folder", value: state.folder ?? "" })}
            >
              {t("sync.change_folder")}
            </Button>
            <Button
              id="sync-leave"
              data-testid="sync-leave"
              variant="ghost"
              size="sm"
              onClick={state.requestLeave}
            >
              {t("sync.leave")}
            </Button>
            <Button
              id="sync-start-over"
              data-testid="sync-start-over"
              variant="ghost"
              size="sm"
              onClick={state.openStartOverModal}
            >
              {t("sync.start_over")}
            </Button>
          </div>

          {state.actionError && prompt === null && (
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

          <div className="flex flex-col gap-1">
            <span className="text-xs font-medium text-m3-on-surface-variant">
              {t("sync.roster_title")}
            </span>
            {state.roster.length === 0 ? (
              <span className="text-xs text-m3-on-surface-variant">{t("sync.roster_empty")}</span>
            ) : (
              <ul id="sync-roster" className="flex flex-col gap-1 text-sm text-m3-on-surface">
                {state.roster.map((entry) => (
                  <li key={entry.deviceId} id={`sync-roster-${entry.deviceId}`}>
                    <span>{entry.deviceName}</span>
                    <span
                      id={`sync-roster-${entry.deviceId}-version`}
                      className="ml-2 text-xs text-m3-on-surface-variant"
                    >
                      {entry.appVersion === null
                        ? t("sync.app_version_unknown")
                        : t("sync.app_version", { version: entry.appVersion })}
                    </span>
                    <span className="ml-2 text-xs text-m3-on-surface-variant">
                      {entry.lastAppliedAt === null
                        ? t("sync.roster_never_applied")
                        : t("sync.roster_last_applied", { when: formatWhen(entry.lastAppliedAt) })}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </div>

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
        </div>
      )}

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

      <Dialog
        id="sync-prompt-dialog"
        isOpen={prompt !== null}
        onClose={() => setPrompt(null)}
        title={
          prompt?.kind === "rename"
            ? t("sync.rename_prompt_title")
            : t("sync.change_folder_prompt_title")
        }
        actions={
          <div className="flex items-center justify-end gap-3">
            <Button variant="ghost" onClick={() => setPrompt(null)}>
              {t("action.cancel")}
            </Button>
            <Button
              id="sync-prompt-submit"
              data-testid="sync-prompt-submit"
              variant="primary"
              disabled={prompt === null || prompt.value.trim() === "" || isSubmittingPrompt}
              loading={isSubmittingPrompt}
              onClick={() => void submitPrompt()}
            >
              {t("action.save")}
            </Button>
          </div>
        }
      >
        {prompt?.kind === "folder" ? (
          <div className="flex items-start gap-2">
            <div className="flex-1">{promptField}</div>
            <Button
              id="sync-prompt-browse"
              data-testid="sync-prompt-browse"
              type="button"
              variant="outline"
              className="mt-6"
              onClick={() => void browseForFolder()}
            >
              {t("sync.browse")}
            </Button>
          </div>
        ) : (
          promptField
        )}
      </Dialog>
    </section>
  );
}
