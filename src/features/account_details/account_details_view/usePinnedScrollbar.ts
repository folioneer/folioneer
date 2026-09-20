import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { computePinnedScrollbar, type PinnedScrollbarGeometry } from "./pinnedScrollbar";

/** Geometry of the holdings scrollbar plus the width the closed section is held at. */
export interface PinnedScrollbarState extends PinnedScrollbarGeometry {
  /** Visible width of the content area; 0 until it has been measured. */
  viewportWidth: number;
}

const UNMEASURED: PinnedScrollbarState = {
  viewportWidth: 0,
  closedStaysPut: false,
  trackOffset: 0,
  scrollRange: 0,
  spacerWidth: 0,
  visible: false,
};

/** Width of a holdings table's pinned block: its Actions and Asset header cells. */
function pinnedWidth(table: HTMLTableElement): number {
  const [actions, asset] = table.querySelectorAll<HTMLElement>("thead th");
  return (actions?.offsetWidth ?? 0) + (asset?.offsetWidth ?? 0);
}

/**
 * #022 — drives the holdings scrollbar: measures the content area whenever it or its
 * tables change size, and keeps the scrollbar and the content area scrolled together.
 * The content area keeps scrolling natively, so the keyboard and the trackpad still work.
 */
export function usePinnedScrollbar() {
  const scrollAreaRef = useRef<HTMLDivElement>(null);
  const scrollbarRef = useRef<HTMLDivElement>(null);
  const [state, setState] = useState<PinnedScrollbarState>(UNMEASURED);

  const measure = useCallback(() => {
    const scrollArea = scrollAreaRef.current;
    const active = scrollArea?.querySelector<HTMLTableElement>("#holdings-table");
    if (!scrollArea || !active) return;
    const closedSection = scrollArea.querySelector<HTMLElement>("#closed-holdings-section");
    const closed = closedSection?.querySelector<HTMLTableElement>("table") ?? null;
    const next: PinnedScrollbarState = {
      viewportWidth: scrollArea.clientWidth,
      ...computePinnedScrollbar({
        viewportWidth: scrollArea.clientWidth,
        contentWidth: scrollArea.scrollWidth,
        activePinnedWidth: pinnedWidth(active),
        // A folded section has no table, yet its header bar is there and has a width.
        closedPinnedWidth: closed ? pinnedWidth(closed) : null,
        closedWidth: closedSection ? closedSection.offsetWidth : null,
      }),
    };
    setState((previous) =>
      (Object.keys(next) as (keyof PinnedScrollbarState)[]).every(
        (key) => previous[key] === next[key],
      )
        ? previous
        : next,
    );
  }, []);

  // Measures before paint, then again whenever the content area or what it holds changes
  // size. The area swaps its children (skeleton, error, tables), so a mutation observer
  // keeps the resize observer pointed at the current ones.
  useLayoutEffect(() => {
    const scrollArea = scrollAreaRef.current;
    if (!scrollArea) return;
    measure();
    if (typeof ResizeObserver === "undefined" || typeof MutationObserver === "undefined") return;
    const resizeObserver = new ResizeObserver(measure);
    const observeChildren = () => {
      resizeObserver.disconnect();
      resizeObserver.observe(scrollArea);
      for (const child of scrollArea.children) resizeObserver.observe(child);
      measure();
    };
    const mutationObserver = new MutationObserver(observeChildren);
    mutationObserver.observe(scrollArea, { childList: true, subtree: true });
    observeChildren();
    return () => {
      mutationObserver.disconnect();
      resizeObserver.disconnect();
    };
  }, [measure]);

  useEffect(() => {
    const scrollArea = scrollAreaRef.current;
    const scrollbar = scrollbarRef.current;
    if (!scrollArea || !scrollbar) return;
    const followScrollbar = () => {
      scrollArea.scrollLeft = scrollbar.scrollLeft;
    };
    const followScrollArea = () => {
      scrollbar.scrollLeft = scrollArea.scrollLeft;
    };
    scrollbar.addEventListener("scroll", followScrollbar);
    scrollArea.addEventListener("scroll", followScrollArea);
    return () => {
      scrollbar.removeEventListener("scroll", followScrollbar);
      scrollArea.removeEventListener("scroll", followScrollArea);
    };
  }, []);

  return { scrollAreaRef, scrollbarRef, scrollbar: state };
}
