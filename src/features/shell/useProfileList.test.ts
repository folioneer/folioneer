import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useProfileList } from "./useProfileList";

const mockList = vi.fn();
vi.mock("./gateway", () => ({
  listProfiles: () => mockList(),
  onProfilesChanged: () => Promise.resolve(() => {}),
}));

describe("useProfileList", () => {
  // #768 — the profiles and the one in use; null when they cannot be read.
  it("reads the profiles, null when they cannot be", async () => {
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
    const { result } = renderHook(() => useProfileList());
    await waitFor(() => expect(result.current.list?.profiles).toHaveLength(2));
    expect(result.current.list?.in_use).toBe("default-profile");
    expect(result.current.error).toBeNull();

    mockList.mockResolvedValueOnce({ status: "error", error: { code: "DatabaseError" } });
    const { result: failed } = renderHook(() => useProfileList());
    await waitFor(() => expect(mockList).toHaveBeenCalledTimes(2));
    expect(failed.current.list).toBeNull();
    await waitFor(() => expect(failed.current.error).toEqual({ key: "error.DatabaseError" }));
  });
});
