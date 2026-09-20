import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ModalContainer } from "./ModalContainer";

interface FormModalProps {
  /** F25 — stable id for the modal panel, targetable from E2E. */
  id?: string;
  isOpen: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  maxWidth?: "max-w-md" | "max-w-2xl" | "max-w-3xl";
  maxHeight?: "max-h-[80vh]" | "max-h-[90vh]";
}

/**
 * FormModal: Header-Content-Footer pattern for forms
 *
 * Use this for forms with 4+ fields or complex layouts that need:
 * - Fixed header with title and close button
 * - Scrollable content area for form fields
 * - Fixed footer for action buttons
 *
 * Best for:
 * - Medium forms (4-6 fields) like EditBankTransferModal
 * - Complex forms (7+ fields with sections) like ProcedureFormModal
 */
export function FormModal({
  id,
  isOpen,
  onClose,
  title,
  children,
  footer,
  maxWidth = "max-w-md",
  maxHeight = "max-h-[90vh]",
}: FormModalProps) {
  const { t } = useTranslation("common");
  return (
    <ModalContainer
      id={id}
      isOpen={isOpen}
      onClose={onClose}
      maxWidth={maxWidth}
      maxHeight={maxHeight}
    >
      {/* Header */}
      <div className="flex items-center justify-between p-6 border-b border-neutral-30">
        <h2 className="text-lg font-semibold text-neutral-90">{title}</h2>
        <button
          type="button"
          onClick={onClose}
          aria-label={t("action.close")}
          id="modal-close-btn"
          className="p-1 hover:bg-neutral-20 rounded-full transition-colors"
        >
          <X size={20} className="text-neutral-70" />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-6 flex flex-col gap-4">{children}</div>

      {/* Footer */}
      {footer && <div className="border-t border-neutral-30 bg-neutral-5 p-4">{footer}</div>}
    </ModalContainer>
  );
}
