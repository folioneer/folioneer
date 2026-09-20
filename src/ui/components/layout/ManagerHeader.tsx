import type { ReactNode } from "react";
import { SearchField } from "../field";

interface ManagerHeaderProps {
  searchId: string;
  searchTerm: string;
  onSearchChange: (value: string) => void;
  searchPlaceholder?: string;
  searchExtra?: ReactNode;
}

export function ManagerHeader({
  searchId,
  searchTerm,
  onSearchChange,
  searchPlaceholder,
  searchExtra,
}: ManagerHeaderProps) {
  return (
    <div className="p-4 flex items-center gap-4">
      <div className="flex-1">
        <SearchField
          id={searchId}
          value={searchTerm}
          onChange={onSearchChange}
          placeholder={searchPlaceholder}
        />
      </div>
      {searchExtra}
    </div>
  );
}
