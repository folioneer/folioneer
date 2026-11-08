import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Profile } from "@/bindings";
import { useProfileNameDialog } from "./useProfileNameDialog";

const mockCreate = vi.fn();
const mockRename = vi.fn();
vi.mock("../gateway", () => ({
  profileGateway: {
    createProfile: (...args: unknown[]) => mockCreate(...args),
    renameProfile: (...args: unknown[]) => mockRename(...args),
  },
}));

const submitEvent = { preventDefault: vi.fn() } as unknown as React.FormEvent;
const parent: Profile = { id: "p-1", name: "Parent" };

describe("useProfileNameDialog", () => {
  beforeEach(() => {
    mockCreate.mockReset();
    mockRename.mockReset();
  });

  // PFL-020 — a new profile; the core's refusal shows and the dialog stays open.
  it("creates a profile, and keeps the refusal on screen", async () => {
    mockCreate.mockResolvedValueOnce({
      status: "error",
      error: { code: "ProfileNameAlreadyExists" },
    });
    const onDone = vi.fn();
    const { result } = renderHook(() =>
      useProfileNameDialog({ isOpen: true, profile: null, onDone }),
    );
    act(() => result.current.setName("Parent"));
    await act(async () => {
      await result.current.submit(submitEvent);
    });
    expect(mockCreate).toHaveBeenCalledWith("Parent");
    expect(result.current.error).toEqual({ key: "error.ProfileNameAlreadyExists" });
    expect(onDone).not.toHaveBeenCalled();

    mockCreate.mockResolvedValueOnce({ status: "ok", data: parent });
    await act(async () => {
      await result.current.submit(submitEvent);
    });
    expect(onDone).toHaveBeenCalled();
  });

  // PFL-030 — a rename starts from the profile's name.
  it("renames a profile, starting from its name", async () => {
    mockRename.mockResolvedValue({ status: "ok", data: parent });
    const onDone = vi.fn();
    const { result } = renderHook(() =>
      useProfileNameDialog({ isOpen: true, profile: parent, onDone }),
    );
    expect(result.current.name).toBe("Parent");
    act(() => result.current.setName("Mum"));
    await act(async () => {
      await result.current.submit(submitEvent);
    });
    expect(mockRename).toHaveBeenCalledWith("p-1", "Mum");
    expect(onDone).toHaveBeenCalled();
  });
});
