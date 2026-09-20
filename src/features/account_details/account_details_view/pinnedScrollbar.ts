/** Measurements of the holdings content area, in CSS pixels (#022). */
export interface PinnedScrollbarMeasures {
  /** Visible width of the content area. */
  viewportWidth: number;
  /** Full width of what the content area scrolls. */
  contentWidth: number;
  /** Width of the active table's pinned block (Actions + Asset). */
  activePinnedWidth: number;
  /** Width of the closed table's pinned block; null when no closed table is shown. */
  closedPinnedWidth: number | null;
  /** Width the closed section needs; null when no closed table is shown. */
  closedWidth: number | null;
}

/** Where the holdings scrollbar sits and how far it travels (#022). */
export interface PinnedScrollbarGeometry {
  /** The closed section fits the window, so it stays put while the active table scrolls. */
  closedStaysPut: boolean;
  /** Left edge of the scrollbar's track: the right edge of the pinned block it skips. */
  trackOffset: number;
  /** How far the content can scroll sideways. */
  scrollRange: number;
  /** Width of the scrollbar's inner spacer, giving it the content's travel over its own track. */
  spacerWidth: number;
  /** False when every column fits and there is nothing to scroll. */
  visible: boolean;
}

/**
 * #022 — the scrollbar covers only the columns that move. Its track starts after the
 * pinned block of the active table; when the closed table is too wide to stay put it
 * scrolls too, and the track starts after the narrower of the two pinned blocks.
 */
export function computePinnedScrollbar(measures: PinnedScrollbarMeasures): PinnedScrollbarGeometry {
  const { viewportWidth, contentWidth, activePinnedWidth, closedPinnedWidth, closedWidth } =
    measures;
  const closedStaysPut = closedWidth !== null && closedWidth <= viewportWidth;
  const trackOffset =
    closedPinnedWidth === null || closedStaysPut || activePinnedWidth < closedPinnedWidth
      ? activePinnedWidth
      : closedPinnedWidth;
  const scrollRange = contentWidth > viewportWidth ? contentWidth - viewportWidth : 0;
  const trackWidth = viewportWidth > trackOffset ? viewportWidth - trackOffset : 0;
  return {
    closedStaysPut,
    trackOffset,
    scrollRange,
    spacerWidth: trackWidth + scrollRange,
    visible: scrollRange > 0,
  };
}
