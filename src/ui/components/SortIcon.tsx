import { ArrowDown, ArrowUp } from "lucide-react";

interface SortIconProps {
  active: boolean;
  direction: "asc" | "desc" | null;
}

export function SortIcon({ active, direction }: SortIconProps) {
  if (!active || !direction) return null;
  return direction === "asc" ? (
    <ArrowUp size={14} className="ml-1 text-m3-primary" />
  ) : (
    <ArrowDown size={14} className="ml-1 text-m3-primary" />
  );
}
