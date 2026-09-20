import { X } from "lucide-react";
import { ModalContainer } from "./ModalContainer";

interface ListModalProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  searchComponent?: React.ReactNode;
  children: React.ReactNode;
  footer?: React.ReactNode;
  maxWidth?: "max-w-2xl" | "max-w-3xl" | "max-w-4xl";
}

/**
 * ListModal: Pattern for filterable/selectable lists with search and stats
 *
 * Use this for:
 * - Searchable lists with filtering
 * - Multi-select with checkboxes (procedures, items)
 * - Lists that need a stats footer showing count/totals
 *
 * Layout:
 * - Fixed header with title and close button
 * - Optional fixed search/filter component
 * - Scrollable list content area
 * - Fixed footer with stats and action buttons
 *
 * Best for:
 * - ProcedureSelectionModal (filtered list + stats)
 * - SelectionModal wrapper cases
 */
export function ListModal({
  isOpen,
  onClose,
  title,
  searchComponent,
  children,
  footer,
  maxWidth = "max-w-3xl",
}: ListModalProps) {
  return (
    <ModalContainer isOpen={isOpen} onClose={onClose} maxWidth={maxWidth} maxHeight="max-h-[80vh]">
      {/* Header */}
      <div className="flex items-center justify-between p-6 border-b border-neutral-30">
        <h2 className="text-lg font-semibold text-neutral-90">{title}</h2>
        <button
          type="button"
          onClick={onClose}
          className="p-1 hover:bg-neutral-20 rounded transition-colors"
        >
          <X size={20} className="text-neutral-70" />
        </button>
      </div>

      {/* Search/Filter Bar (Optional, Fixed) */}
      {searchComponent && (
        <div className="flex-shrink-0 px-6 pt-4 pb-2 border-b border-neutral-30">
          {searchComponent}
        </div>
      )}

      {/* List Content (Scrollable) */}
      <div className="flex-1 overflow-y-auto p-6 flex flex-col gap-4">{children}</div>

      {/* Stats/Footer (Fixed) */}
      {footer && <div className="border-t border-neutral-30 bg-neutral-5 p-4">{footer}</div>}
    </ModalContainer>
  );
}
