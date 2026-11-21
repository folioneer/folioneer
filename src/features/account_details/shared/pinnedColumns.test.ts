import { describe, expect, it } from "vitest";
import {
  PINNED_ACTIONS_CELL,
  PINNED_ACTIONS_HEADER,
  PINNED_ASSET_CELL,
  PINNED_ASSET_HEADER,
} from "./pinnedColumns";

// #712 (the validated mock) — the actions column fits the twelve actions a row can show: six
// grid columns of 32px buttons, five 4px gaps and the cell's 24px padding on each side.
describe("pinned columns (#712)", () => {
  const width = 6 * 32 + 5 * 4 + 2 * 24;

  it("gives the actions column room for six columns of buttons", () => {
    for (const actions of [PINNED_ACTIONS_HEADER, PINNED_ACTIONS_CELL]) {
      expect(actions).toContain(`w-[${width}px]`);
      expect(actions).toContain(`max-w-[${width}px]`);
    }
  });

  it("pins the asset column right after it", () => {
    for (const asset of [PINNED_ASSET_HEADER, PINNED_ASSET_CELL]) {
      expect(asset).toContain(`left-[${width}px]`);
    }
  });
});
