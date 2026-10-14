import { useEffect } from "react";

/**
 * AGT-032 / AGT-083 — while the dialog `dialogId` is open, Tab moves between its actions
 * and never to what lies behind it.
 */
export function useKeepFocusOnActions(dialogId: string): void {
  useEffect(() => {
    const keepFocus = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return;
      const actions = Array.from(
        document.querySelectorAll<HTMLButtonElement>(`#${dialogId} button:not(:disabled)`),
      );
      if (actions.length === 0) return;
      event.preventDefault();
      const current = actions.indexOf(document.activeElement as HTMLButtonElement);
      const step = event.shiftKey ? -1 : 1;
      actions[(current + step + actions.length) % actions.length]?.focus();
    };
    document.addEventListener("keydown", keepFocus, true);
    return () => document.removeEventListener("keydown", keepFocus, true);
  }, [dialogId]);
}
