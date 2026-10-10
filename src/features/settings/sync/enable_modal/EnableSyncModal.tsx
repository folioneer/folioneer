import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { TextField } from "@/ui/components/field/TextField";
import { ConfirmationDialog } from "@/ui/components/modal/Dialog";
import { FormModal } from "@/ui/components/modal/FormModal";
import { useEnableSyncModal } from "./useEnableSyncModal";

interface EnableSyncModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  /** `start-over` re-creates the portfolio in the folder under a new passphrase (SYN-071); the join refusal of `enable` offers it too (SYN-053). */
  variant: "enable" | "start-over";
}

/** SYN-012 — advisory only: a length-based hint that never blocks submission. */
function strengthKey(passphrase: string): string {
  if (passphrase.length < 16) return "sync.enable_modal.strength_weak";
  if (passphrase.length < 24) return "sync.enable_modal.strength_fair";
  return "sync.enable_modal.strength_strong";
}

/** SYN-011/071 — what the passphrase step says of the folder and of the passphrase. */
function passphraseWordingKey(
  isStartOver: boolean,
  isJoin: boolean,
  folderHoldsPortfolio: boolean,
): string {
  if (isStartOver && folderHoldsPortfolio) return "sync.enable_modal.start_over_wording";
  return isJoin ? "sync.enable_modal.join_wording" : "sync.enable_modal.first_device_wording";
}

/**
 * SYN-011/012/014/015/017/019/053/054/071 — two-step enable modal: the folder
 * (typed or browsed, validated on change), then the passphrase — once to join,
 * twice otherwise — with the device name and the honesty statements.
 */
export function EnableSyncModal({ isOpen, onClose, onSuccess, variant }: EnableSyncModalProps) {
  const { t } = useTranslation();
  const state = useEnableSyncModal({ variant, onSuccess });
  const isStartOver = state.isStartOver;

  const submitLabel = isStartOver
    ? t("sync.enable_modal.submit_start_over")
    : state.isJoin
      ? t("sync.enable_modal.submit_join")
      : t("sync.enable_modal.submit");

  const footer =
    state.step === 1 ? (
      <div className="flex items-center justify-end gap-3">
        <Button variant="ghost" onClick={onClose}>
          {t("action.cancel")}
        </Button>
        <Button
          id="sync-enable-next"
          data-testid="sync-enable-next"
          variant="primary"
          disabled={!state.canProceedToStep2}
          onClick={state.goToStep2}
        >
          {t("sync.enable_modal.next")}
        </Button>
      </div>
    ) : (
      <div className="flex items-center justify-end gap-3">
        <Button variant="ghost" onClick={onClose} disabled={state.isSubmitting}>
          {t("action.cancel")}
        </Button>
        <Button
          id="sync-enable-submit"
          data-testid="sync-enable-submit"
          type="submit"
          form="sync-enable-form"
          variant={isStartOver ? "danger" : "primary"}
          disabled={!state.canSubmit}
          loading={state.isSubmitting}
        >
          {state.isSubmitting ? t("sync.enable_modal.submitting") : submitLabel}
        </Button>
      </div>
    );

  return (
    <>
      <FormModal
        id="sync-enable-modal"
        isOpen={isOpen}
        onClose={onClose}
        title={isStartOver ? t("sync.enable_modal.title_start_over") : t("sync.enable_modal.title")}
        footer={footer}
      >
        <form
          id="sync-enable-form"
          data-testid="sync-enable-form"
          className="flex flex-col gap-4"
          onSubmit={(event) => void state.handleSubmit(event)}
        >
          <p className="text-sm text-m3-on-surface-variant">{t("sync.local_copy_note")}</p>

          {state.step === 1 ? (
            <>
              <span className="text-sm font-medium text-m3-on-surface">
                {t("sync.enable_modal.step_folder")}
              </span>
              <div className="flex items-start gap-2">
                <div className="flex-1">
                  <TextField
                    id="sync-enable-folder"
                    data-testid="sync-enable-folder"
                    label={t("sync.enable_modal.folder_label")}
                    placeholder={t("sync.enable_modal.folder_placeholder")}
                    value={state.folder}
                    onChange={(event) => void state.setFolder(event.target.value)}
                    error={
                      state.folderError && !state.joinRefused
                        ? t(state.folderError.key, state.folderError.vars)
                        : undefined
                    }
                  />
                </div>
                <Button
                  id="sync-enable-browse"
                  data-testid="sync-enable-browse"
                  type="button"
                  variant="outline"
                  className="mt-6"
                  onClick={() => void state.handleBrowse()}
                >
                  {t("sync.browse")}
                </Button>
              </div>
              {state.joinRefused && (
                <div className="flex flex-col gap-2 rounded-2xl border border-m3-error p-3 text-sm">
                  <div id="sync-enable-join-refused" role="alert" className="flex flex-col gap-2">
                    <span className="text-m3-error">{t("sync.join_refused.title")}</span>
                    <span className="text-m3-on-surface">{t("sync.join_refused.why")}</span>
                    <ol className="list-decimal pl-5 text-m3-on-surface">
                      <li>{t("sync.join_refused.step_keep")}</li>
                      <li>{t("sync.join_refused.step_rename")}</li>
                      <li>{t("sync.join_refused.step_join")}</li>
                    </ol>
                    <code className="whitespace-pre-line break-all text-xs text-m3-on-surface-variant">
                      {t("sync.join_refused.where")}
                    </code>
                  </div>
                  {/* SYN-053 — a computer that holds the portfolio may start over instead */}
                  <div className="mt-1 flex flex-col items-start gap-2 border-t border-m3-outline-variant pt-3">
                    <span id="sync-enable-start-over-offer" className="text-m3-on-surface">
                      {t("sync.join_refused.start_over")}
                    </span>
                    <Button
                      id="sync-enable-start-over"
                      data-testid="sync-enable-start-over"
                      type="button"
                      variant="outline"
                      size="sm"
                      aria-describedby="sync-enable-start-over-offer"
                      onClick={state.startOverInstead}
                    >
                      {t("sync.start_over")}
                    </Button>
                  </div>
                </div>
              )}
              <p className="text-xs text-m3-on-surface-variant">
                {t("sync.enable_modal.folder_hint")}
              </p>
            </>
          ) : (
            <>
              <span className="text-sm font-medium text-m3-on-surface">
                {t("sync.enable_modal.step_passphrase")}
              </span>
              <p className="text-sm text-m3-on-surface">
                {t(passphraseWordingKey(isStartOver, state.isJoin, state.folderHoldsPortfolio))}
              </p>
              <TextField
                id="sync-enable-passphrase"
                data-testid="sync-enable-passphrase"
                type="password"
                label={t("sync.enable_modal.passphrase_label")}
                value={state.passphrase}
                onChange={(event) => state.setPassphrase(event.target.value)}
                required
                error={
                  state.passphraseTooShort ? t("sync.enable_modal.passphrase_too_short") : undefined
                }
              />
              {!state.isJoin && state.passphrase.length > 0 && !state.passphraseTooShort && (
                <span id="sync-enable-strength" className="text-xs text-m3-on-surface-variant">
                  {t(strengthKey(state.passphrase))}
                </span>
              )}
              {!state.isJoin && (
                <TextField
                  id="sync-enable-passphrase-confirm"
                  data-testid="sync-enable-passphrase-confirm"
                  type="password"
                  label={t("sync.enable_modal.passphrase_confirm_label")}
                  value={state.passphraseConfirm}
                  onChange={(event) => state.setPassphraseConfirm(event.target.value)}
                  required
                  error={
                    state.passphraseMismatch
                      ? t("sync.enable_modal.passphrase_mismatch")
                      : undefined
                  }
                />
              )}
              <TextField
                id="sync-enable-device-name"
                data-testid="sync-enable-device-name"
                label={t("sync.enable_modal.device_name_label")}
                placeholder={t("sync.enable_modal.device_name_placeholder")}
                value={state.deviceName}
                onChange={(event) => state.setDeviceName(event.target.value)}
                required
              />
              <p className="text-xs text-m3-on-surface-variant">{t("sync.no_recovery_note")}</p>
              <p className="text-xs text-m3-on-surface-variant">
                {t("sync.metadata_exposure_note")}
              </p>
              {state.submitError && (
                <p id="sync-enable-error" className="text-sm text-m3-error">
                  {t(state.submitError.key, state.submitError.vars)}
                </p>
              )}
            </>
          )}
        </form>
      </FormModal>

      <ConfirmationDialog
        isOpen={state.confirmingStartOver}
        onCancel={state.cancelStartOver}
        onConfirm={() => void state.confirmStartOver()}
        title={t("sync.start_over_confirm_title")}
        message={t("sync.start_over_confirm_message")}
        confirmLabel={t("sync.start_over")}
        cancelLabel={t("action.cancel")}
        variant="danger"
        confirmId="sync-start-over-confirm"
      />
    </>
  );
}
