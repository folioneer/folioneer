import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useManageProfiles } from "./useManageProfiles";

const mockList = vi.fn();
const mockRemove = vi.fn();
vi.mock("../gateway", () => ({
  profileGateway: {
    listProfiles: () => mockList(),
    removeProfile: (...args: unknown[]) => mockRemove(...args),
    subscribeToEvents: () => Promise.resolve(() => {}),
  },
}));

describe("useManageProfiles", () => {
  beforeEach(() => {
    mockList.mockReset();
    mockRemove.mockReset();
    mockList.mockResolvedValue({
      status: "ok",
      data: {
        profiles: [
          { profile: { id: "default-profile", name: null }, account_count: 2, removable: false },
          { profile: { id: "p-1", name: "Parent" }, account_count: 0, removable: true },
        ],
        in_use: "default-profile",
      },
    });
  });

  // PFL-023 — read only once the dialog opens.
  it("reads the profiles once open", async () => {
    const { result, rerender } = renderHook(({ open }) => useManageProfiles(open), {
      initialProps: { open: false },
    });
    expect(mockList).not.toHaveBeenCalled();
    rerender({ open: true });
    await waitFor(() => expect(result.current.profiles).toHaveLength(2));
  });

  // PFL-040 — a removal waits for the confirmation; a refused one is kept as the error to show.
  it("keeps a refused removal as the error to show", async () => {
    mockRemove.mockResolvedValue({ status: "error", error: { code: "ProfileNotEmpty" } });
    const { result } = renderHook(() => useManageProfiles(true));
    await waitFor(() => expect(result.current.profiles).toHaveLength(2));
    act(() => result.current.requestRemoval({ id: "p-1", name: "Parent" }));
    expect(mockRemove).not.toHaveBeenCalled();
    await act(async () => {
      await result.current.confirmRemoval();
    });
    expect(mockRemove).toHaveBeenCalledWith("p-1");
    expect(result.current.error).toEqual({ key: "error.ProfileNotEmpty" });
  });

  // PFL-020 / PFL-030 — the name dialog opens to add, or to rename a profile.
  it("opens the name dialog to add or to rename", () => {
    const { result } = renderHook(() => useManageProfiles(false));
    act(() => result.current.openRename({ id: "p-1", name: "Parent" }));
    expect(result.current.nameDialog).toEqual({
      open: true,
      profile: { id: "p-1", name: "Parent" },
    });
    act(() => result.current.openAdd());
    expect(result.current.nameDialog).toEqual({ open: true, profile: null });
    act(() => result.current.closeNameDialog());
    expect(result.current.nameDialog).toEqual({ open: false });
  });
});
