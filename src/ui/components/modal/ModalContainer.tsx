import { type ReactNode, useEffect } from "react";
import { useTranslation } from "react-i18next";

interface ModalContainerProps {
  /** F25 — stable id for the dialog panel, targetable from E2E. */
  id?: string;
  isOpen: boolean;
  onClose: () => void;
  children: ReactNode;
  maxWidth?: "max-w-md" | "max-w-2xl" | "max-w-3xl" | "max-w-4xl";
  maxHeight?: "max-h-[80vh]" | "max-h-[90vh]";
  titleId?: string;
}

/**
 * ModalContainer: Base modal wrapper with consistent overlay and close handling
 *
 * This is the foundation for all modal patterns. It provides:
 * - Fixed overlay with centered positioning
 * - Consistent backdrop styling
 * - Body scroll prevention
 * - Escape key handling
 *
 * Use this for simple modals or as a wrapper for more complex patterns.
 */
export function ModalContainer({
  id,
  isOpen,
  onClose,
  children,
  maxWidth = "max-w-md",
  maxHeight = "max-h-[90vh]",
  titleId,
}: ModalContainerProps) {
  const { t } = useTranslation("common");

  useEffect(() => {
    const handleEscapeKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && isOpen) {
        onClose();
      }
    };

    if (isOpen) {
      document.body.style.overflow = "hidden";
      document.addEventListener("keydown", handleEscapeKey);
    }

    return () => {
      if (isOpen) {
        document.body.style.overflow = "auto";
      }
      document.removeEventListener("keydown", handleEscapeKey);
    };
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop — interactive button so screen readers can dismiss */}
      <button
        type="button"
        aria-label={t("action.close")}
        className="absolute inset-0 bg-m3-scrim/50 backdrop-blur-[2px] cursor-default"
        onClick={onClose}
      />
      {/* Dialog panel — sibling, renders above backdrop via DOM order */}
      <div
        id={id}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className={`relative bg-m3-surface-container-lowest/85 backdrop-blur-[12px] rounded-[28px] shadow-elevation-4 w-full ${maxWidth} ${maxHeight} overflow-hidden flex flex-col`}
      >
        {children}
      </div>
    </div>
  );
}
