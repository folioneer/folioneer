import { useCallback, useEffect, useRef } from "react";

/** The open modals, the last opened last. */
const openModals: object[] = [];

/**
 * Enters an open modal in the stack `ModalContainer` and `Dialog` share: the page does not
 * scroll while any is open, and the answer says whether this one is on top (Escape is the top
 * one's alone).
 */
export function useModalStack(isOpen: boolean): () => boolean {
  const self = useRef({});

  useEffect(() => {
    if (!isOpen) return;
    const modal = self.current;
    openModals.push(modal);
    document.body.style.overflow = "hidden";
    return () => {
      openModals.splice(openModals.indexOf(modal), 1);
      if (openModals.length === 0) document.body.style.overflow = "auto";
    };
  }, [isOpen]);

  return useCallback(() => openModals.at(-1) === self.current, []);
}
