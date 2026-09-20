import type React from "react";

interface ContentProps {
  children: React.ReactNode;
}

export function Content({ children }: ContentProps) {
  return <main className="flex-1 overflow-hidden flex flex-col">{children}</main>;
}
