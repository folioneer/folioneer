import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useProfileChip } from "./useProfileChip";

const mockList = vi.fn();
const mockSetInUse = vi.fn();
let emit: (type: string) => void = () => {};
vi.mock("../gateway", () => ({
  listProfiles: () => mockList(),
  setProfileInUse: (...args: unknown[]) => mockSetInUse(...args),
  onProfilesChanged: (callback: () => void) => {
    emit = () => callback();
    return Promise.resolve(() => {});
  },
}));
const mockNavigate = vi.fn();
vi.mock("@tanstack/react-router", () => ({ useNavigate: () => mockNavigate }));

const twoProfiles = {
  status: "ok",
  data: {
    profiles: [
      { profile: { id: "default-profile", name: null }, account_count: 2, removable: false },
      { profile: { id: "p-1", name: "Parent" }, account_count: 0, removable: true },
    ],
    in_use: "default-profile",
  },
};

describe("useProfileChip", () => {
  beforeEach(() => {
    mockList.mockReset();
    mockSetInUse.mockReset();
    mockNavigate.mockReset();
    mockList.mockResolvedValue(twoProfiles);
  });

  // PFL-012 / PFL-023 — the profiles and the one in use, read again on ProfileUpdated.
  it("reads the profiles and the one in use, again on ProfileUpdated", async () => {
    const { result } = renderHook(() => useProfileChip());
    await waitFor(() => expect(result.current.inUse?.id).toBe("default-profile"));
    expect(result.current.profiles).toHaveLength(2);
    act(() => emit("ProfileUpdated"));
    await waitFor(() => expect(mockList).toHaveBeenCalledTimes(2));
  });

  // PFL-032 / PFL-032A — another profile is chosen and the window returns to the account
  // list; the one in use chosen again calls nothing.
  it("chooses another profile, never the one in use again", async () => {
    mockSetInUse.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useProfileChip());
    await waitFor(() => expect(result.current.inUse).not.toBeNull());
    await act(async () => {
      await result.current.choose("default-profile");
    });
    expect(mockSetInUse).not.toHaveBeenCalled();
    await act(async () => {
      await result.current.choose("p-1");
    });
    expect(mockSetInUse).toHaveBeenCalledWith("p-1");
    expect(mockNavigate).toHaveBeenCalledWith({ to: "/accounts" });
  });

  // F27 — a refused choice is kept as the error to show.
  it("keeps a refused choice as the error to show", async () => {
    mockSetInUse.mockResolvedValue({ status: "error", error: { code: "ProfileNotFound" } });
    const { result } = renderHook(() => useProfileChip());
    await waitFor(() => expect(result.current.inUse).not.toBeNull());
    await act(async () => {
      await result.current.choose("p-1");
    });
    expect(result.current.error).toEqual({ key: "error.ProfileNotFound" });
    expect(mockNavigate).not.toHaveBeenCalled();
  });
});
