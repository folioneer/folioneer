import { useCallback, useRef, useState } from "react";
import type { SyncFolderState } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";
import { enableSync, inspectSyncFolder, pickSyncFolder, startSyncOver } from "../../gateway";
import { folderProblemToI18n, syncErrorToI18n } from "../../shared/presenter";

/** SYN-012 — the shortest passphrase the backend accepts. */
export const PASSPHRASE_MINIMUM_LENGTH = 12;

export interface UseEnableSyncModalOptions {
  /** `start-over` asks its own confirmation before calling `start_sync_over` (SYN-071). */
  variant: "enable" | "start-over";
  onSuccess?: () => void;
}

export interface UseEnableSyncModalResult {
  step: 1 | 2;
  folder: string;
  setFolder: (folder: string) => Promise<void>;
  handleBrowse: () => Promise<void>;
  /** Why the folder cannot be used, from `inspect_sync_folder` (SYN-014/019/035). */
  folderError: I18nMessage | null;
  canProceedToStep2: boolean;
  goToStep2: () => void;
  passphrase: string;
  setPassphrase: (value: string) => void;
  passphraseConfirm: string;
  setPassphraseConfirm: (value: string) => void;
  passphraseMismatch: boolean;
  passphraseTooShort: boolean;
  deviceName: string;
  setDeviceName: (value: string) => void;
  /** True when the folder already holds a portfolio: one passphrase field, join wording (SYN-011). */
  isJoin: boolean;
  canSubmit: boolean;
  isSubmitting: boolean;
  submitError: I18nMessage | null;
  handleSubmit: (event?: { preventDefault: () => void }) => Promise<void>;
  confirmingStartOver: boolean;
  confirmStartOver: () => Promise<void>;
  cancelStartOver: () => void;
}

/** SYN-014/019/035 — the first reason the inspected folder cannot be used, if any. */
function folderStateError(state: SyncFolderState): I18nMessage | null {
  if (state.problem !== null) {
    return folderProblemToI18n(state.problem);
  }
  if (state.holds_portfolio && !state.format_readable) {
    return {
      key: "sync.errors.UpdateRequired",
      vars: { dataFormatVersion: state.data_format_version ?? 0 },
    };
  }
  if (state.holds_portfolio && state.installation_holds_user_data) {
    return { key: "sync.errors.InstallationHoldsUserData" };
  }
  return null;
}

/**
 * SYN-011/012/014/015/018/019/071 — two-step enable flow: the folder is inspected
 * on every change and decides the wording of step 2 (first device vs join);
 * the passphrase is typed twice only for a first device.
 */
export function useEnableSyncModal({
  variant,
  onSuccess,
}: UseEnableSyncModalOptions): UseEnableSyncModalResult {
  const [step, setStep] = useState<1 | 2>(1);
  const [folder, setFolderValue] = useState("");
  const [folderState, setFolderState] = useState<SyncFolderState | null>(null);
  const [folderError, setFolderError] = useState<I18nMessage | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const [passphraseConfirm, setPassphraseConfirm] = useState("");
  const [deviceName, setDeviceName] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<I18nMessage | null>(null);
  const [confirmingStartOver, setConfirmingStartOver] = useState(false);
  const inspectSequence = useRef(0);

  const setFolder = useCallback(async (value: string) => {
    setFolderValue(value);
    setFolderState(null);
    const sequence = ++inspectSequence.current;
    if (value.trim() === "") {
      setFolderError(null);
      return;
    }
    const result = await inspectSyncFolder(value);
    if (sequence !== inspectSequence.current) {
      return;
    }
    if (result.status === "ok") {
      setFolderState(result.data);
      setFolderError(folderStateError(result.data));
    } else {
      setFolderError(syncErrorToI18n(result.error));
    }
  }, []);

  const handleBrowse = useCallback(async () => {
    const picked = await pickSyncFolder();
    if (picked !== null) {
      await setFolder(picked);
    }
  }, [setFolder]);

  const isJoin = folderState?.holds_portfolio === true;
  const canProceedToStep2 = folderState !== null && folderError === null;
  const passphraseTooShort = passphrase.length > 0 && passphrase.length < PASSPHRASE_MINIMUM_LENGTH;
  const passphraseMismatch =
    !isJoin && passphraseConfirm !== "" && passphraseConfirm !== passphrase;
  const canSubmit =
    canProceedToStep2 &&
    passphrase.length >= PASSPHRASE_MINIMUM_LENGTH &&
    (isJoin || passphraseConfirm === passphrase) &&
    deviceName.trim() !== "" &&
    !isSubmitting;

  const run = useCallback(async () => {
    setIsSubmitting(true);
    setSubmitError(null);
    const call = variant === "start-over" ? startSyncOver : enableSync;
    const result = await call(folder, passphrase, deviceName.trim());
    if (result.status === "ok") {
      onSuccess?.();
    } else {
      setSubmitError(syncErrorToI18n(result.error));
    }
    setIsSubmitting(false);
  }, [variant, folder, passphrase, deviceName, onSuccess]);

  const handleSubmit = useCallback(
    async (event?: { preventDefault: () => void }) => {
      event?.preventDefault();
      if (!canSubmit) {
        return;
      }
      if (variant === "start-over") {
        setConfirmingStartOver(true);
        return;
      }
      await run();
    },
    [canSubmit, variant, run],
  );

  const confirmStartOver = useCallback(async () => {
    setConfirmingStartOver(false);
    await run();
  }, [run]);

  return {
    step,
    folder,
    setFolder,
    handleBrowse,
    folderError,
    canProceedToStep2,
    goToStep2: () => setStep(2),
    passphrase,
    setPassphrase,
    passphraseConfirm,
    setPassphraseConfirm,
    passphraseMismatch,
    passphraseTooShort,
    deviceName,
    setDeviceName,
    isJoin,
    canSubmit,
    isSubmitting,
    submitError,
    handleSubmit,
    confirmingStartOver,
    confirmStartOver,
    cancelStartOver: () => setConfirmingStartOver(false),
  };
}
