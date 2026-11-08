import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useProfileCount } from "./useProfileCount";

const mockList = vi.fn();
vi.mock("./gateway", () => ({
  listProfiles: () => mockList(),
  onProfilesChanged: () => Promise.resolve(() => {}),
}));

describe("useProfileCount", () => {
  // #768 — how many profiles there are; 0 when they cannot be read.
  it("counts the profiles, 0 when they cannot be read", async () => {
    mockList.mockResolvedValueOnce({
      status: "ok",
      data: {
        profiles: [
          { profile: { id: "default-profile", name: null }, account_count: 1, removable: false },
          { profile: { id: "p-1", name: "Parent" }, account_count: 0, removable: true },
        ],
        in_use: "default-profile",
      },
    });
    const { result } = renderHook(() => useProfileCount());
    await waitFor(() => expect(result.current).toBe(2));

    mockList.mockResolvedValueOnce({ status: "error", error: { code: "DatabaseError" } });
    const { result: failed } = renderHook(() => useProfileCount());
    await waitFor(() => expect(mockList).toHaveBeenCalledTimes(2));
    expect(failed.current).toBe(0);
  });
});
