import { X } from "lucide-react";
import type React from "react";
import { useEffect, useId } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../button";

interface DialogProps {
  /** F25 — stable id for the dialog surface, targetable from E2E. */
  id?: string;
  isOpen: boolean;
  onClose: () => void;
  title: React.ReactNode;
  children: React.ReactNode;
  actions?: React.ReactNode;
  maxWidth?: string;
  disableClose?: boolean;
}

export function Dialog({
  id,
  isOpen,
  onClose,
  title,
  children,
  actions,
  maxWidth = "max-w-md",
  disableClose = false,
}: DialogProps) {
  const { t } = useTranslation("common");
  const titleId = useId();
  // Prevent scrolling when dialog is open
  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = "hidden";
    } else {
      document.body.style.overflow = "unset";
    }
    return () => {
      document.body.style.overflow = "unset";
    };
  }, [isOpen]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-100 flex items-center justify-center p-4">
      {/* Backdrop */}
      <button
        type="button"
        aria-hidden="true"
        tabIndex={-1}
        className="absolute inset-0 w-full h-full bg-m3-on-surface/40 backdrop-blur-[2px] transition-opacity cursor-default border-none outline-none"
        onClick={disableClose ? undefined : onClose}
      />

      {/* Dialog Surface */}
      <div
        id={id}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className={`relative w-full ${maxWidth} max-h-[calc(100vh-2rem)] bg-m3-surface-container-lowest/85 backdrop-blur-md rounded-[28px] shadow-elevation-4 flex flex-col overflow-hidden animate-in fade-in zoom-in duration-200`}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4">
          <h3 id={titleId} className="text-xl font-medium text-m3-on-surface">
            {title}
          </h3>
          {!disableClose && (
            <button
              type="button"
              onClick={onClose}
              aria-label={t("action.close")}
              id="modal-close-btn"
              className="p-2 hover:bg-m3-on-surface/5 rounded-full text-m3-on-surface-variant transition-colors"
            >
              <X size={20} />
            </button>
          )}
        </div>

        {/* Content */}
        <div className="px-6 py-2 overflow-y-auto text-m3-on-surface-variant flex-1 min-h-0">
          {children}
        </div>

        {/* Footer Actions */}
        {actions && (
          <div className="flex items-center justify-end gap-2 px-6 py-4 mt-2">{actions}</div>
        )}
      </div>
    </div>
  );
}

interface ConfirmationDialogProps {
  isOpen: boolean;
  onCancel: () => void;
  onConfirm: () => void;
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel: string;
  variant?: "default" | "danger";
  confirmId?: string;
}

export function ConfirmationDialog({
  isOpen,
  onCancel,
  onConfirm,
  title,
  message,
  confirmLabel,
  cancelLabel,
  variant = "default",
  confirmId,
}: ConfirmationDialogProps) {
  const actions = (
    <div className="flex items-center justify-end gap-3">
      <Button variant="ghost" onClick={onCancel}>
        {cancelLabel}
      </Button>
      <Button
        variant={variant === "danger" ? "danger" : "primary"}
        id={confirmId}
        data-testid={confirmId}
        onClick={onConfirm}
      >
        {confirmLabel}
      </Button>
    </div>
  );

  return (
    <Dialog
      id={confirmId ? `${confirmId}-dialog` : undefined}
      isOpen={isOpen}
      onClose={onCancel}
      title={title}
      actions={actions}
    >
      <p className="text-m3-on-surface-variant leading-relaxed">{message}</p>
    </Dialog>
  );
}
