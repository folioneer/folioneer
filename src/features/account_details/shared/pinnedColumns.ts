/**
 * #001 — the Actions and Asset columns stay pinned at the left edge of the holdings
 * tables while the view's content area scrolls sideways. Actions has a fixed width —
 * nine 32px buttons in five grid columns, their gaps and the cell padding — so Asset
 * can be pinned right after it. Pinned cells carry an opaque background (tinted like
 * the row on hover) so scrolled columns never show through, and a 1px line drawn
 * inside the Asset cell marks the pinned edge: a collapsed-table border does not
 * travel with a sticky cell.
 */
const PINNED_EDGE =
  "after:absolute after:inset-y-0 after:right-0 after:w-px after:bg-m3-outline/40 after:content-['']";
const PINNED_ROW_BACKGROUND =
  "bg-m3-surface-container-low group-hover:bg-[color-mix(in_srgb,var(--color-m3-on-surface)_6%,var(--color-m3-surface-container-low))]";

export const PINNED_ACTIONS_HEADER =
  "sticky left-0 z-30 w-[224px] min-w-[224px] max-w-[224px] hover:bg-m3-surface-container-low";
export const PINNED_ASSET_HEADER = `sticky left-[224px] z-30 hover:bg-m3-surface-container-low ${PINNED_EDGE}`;
export const PINNED_ACTIONS_CELL = `sticky left-0 z-10 w-[224px] min-w-[224px] max-w-[224px] ${PINNED_ROW_BACKGROUND}`;
export const PINNED_ASSET_CELL = `sticky left-[224px] z-10 ${PINNED_ROW_BACKGROUND} ${PINNED_EDGE}`;
