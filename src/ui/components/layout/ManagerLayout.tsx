import type { ReactNode } from "react";
import { ManagerHeader } from "./ManagerHeader";

interface ManagerLayoutProps {
  // Left panel (list)
  searchId: string;
  searchTerm: string;
  onSearchChange: (value: string) => void;
  searchPlaceholder?: string;
  searchExtra?: ReactNode;
  table: ReactNode;

  // Right panel (action form) — optional
  sidePanelTitle?: string;
  sidePanelIcon?: ReactNode;
  sidePanelContent?: ReactNode;
  sidePanelDescription?: string;
}

export function ManagerLayout({
  searchId,
  searchTerm,
  onSearchChange,
  searchPlaceholder,
  searchExtra,
  table,
  sidePanelTitle,
  sidePanelIcon,
  sidePanelContent,
  sidePanelDescription,
}: ManagerLayoutProps) {
  const hasSidePanel = !!sidePanelContent;

  return (
    <div className="flex h-full gap-4 overflow-hidden py-2 px-2">
      {/* Left List */}
      <div className="flex-1 flex flex-col min-w-0 bg-m3-surface-container rounded-[28px] shadow-elevation-1 overflow-hidden">
        <ManagerHeader
          searchId={searchId}
          searchTerm={searchTerm}
          onSearchChange={onSearchChange}
          searchPlaceholder={searchPlaceholder}
          searchExtra={searchExtra}
        />
        <div className="flex-1 overflow-auto">{table}</div>
      </div>

      {/* Right add form — only rendered when sidePanelContent is provided */}
      {hasSidePanel && (
        <div className="w-96 flex flex-col bg-m3-surface-container-high rounded-4xl shadow-elevation-1 shrink-0 overflow-hidden">
          <div className="p-4 bg-m3-surface-container-highest">
            <div className="flex items-center gap-4 mb-1">
              <div className="p-2 bg-m3-primary-container text-m3-on-primary-container rounded-xl">
                {sidePanelIcon}
              </div>
              <h2 className="text-xl font-bold text-m3-on-surface">{sidePanelTitle}</h2>
            </div>
            {sidePanelDescription && (
              <p className="text-sm text-m3-on-surface-variant ml-14">{sidePanelDescription}</p>
            )}
          </div>
          <div className="p-4 overflow-y-auto custom-scrollbar flex-1">{sidePanelContent}</div>
        </div>
      )}
    </div>
  );
}
