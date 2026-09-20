import { describe, expect, it, vi } from "vitest";
import { patchModalSearch } from "./modalSearch";

type NavigateArg = { search: (prev: Record<string, unknown>) => unknown; replace?: boolean };

describe("patchModalSearch", () => {
  it("merges the patch onto the previous search params", () => {
    const navigate = vi.fn();

    patchModalSearch(navigate as unknown as Parameters<typeof patchModalSearch>[0], {
      modal: "edit-asset",
      editAssetId: "asset-1",
      focusField: "reference",
    });

    const arg = navigate.mock.calls[0]?.[0] as NavigateArg;
    expect(arg.replace).toBeUndefined();
    expect(arg.search({ existing: "kept" })).toEqual({
      existing: "kept",
      modal: "edit-asset",
      editAssetId: "asset-1",
      focusField: "reference",
    });
  });

  it("forwards the replace option", () => {
    const navigate = vi.fn();

    patchModalSearch(
      navigate as unknown as Parameters<typeof patchModalSearch>[0],
      { modal: undefined, editAssetId: undefined, focusField: undefined },
      { replace: true },
    );

    const arg = navigate.mock.calls[0]?.[0] as NavigateArg;
    expect(arg.replace).toBe(true);
    expect(arg.search({ modal: "edit-asset", editAssetId: "asset-1" })).toEqual({
      modal: undefined,
      editAssetId: undefined,
      focusField: undefined,
    });
  });
});
