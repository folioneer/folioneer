import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProfileChip } from "./ProfileChip";

const mockList = vi.fn();
const mockSetInUse = vi.fn();
vi.mock("../gateway", () => ({
  listProfiles: () => mockList(),
  setProfileInUse: (...args: unknown[]) => mockSetInUse(...args),
  onProfilesChanged: () => Promise.resolve(() => {}),
}));
const mockNavigate = vi.fn();
vi.mock("@tanstack/react-router", () => ({ useNavigate: () => mockNavigate }));
// The hosted dialogs are the profiles feature's; they stay closed here.
vi.mock("@/features/profiles", () => ({
  ManageProfilesDialog: ({ isOpen }: { isOpen: boolean }) =>
    isOpen ? <div id="manage-open" /> : null,
  ProfileNameDialog: ({ isOpen }: { isOpen: boolean }) => (isOpen ? <div id="add-open" /> : null),
}));

const profiles = (count: number) => ({
  status: "ok",
  data: {
    profiles: [
      { profile: { id: "default-profile", name: null }, account_count: 2, removable: false },
      { profile: { id: "p-1", name: "Parent" }, account_count: 0, removable: true },
    ].slice(0, count),
    in_use: "default-profile",
  },
});

describe("ProfileChip", () => {
  beforeEach(() => {
    mockList.mockReset();
    mockSetInUse.mockReset();
    mockNavigate.mockReset();
  });

  // #767 — one profile: the menu offers to add one, which opens its dialog.
  it("offers to add a profile when there is one", async () => {
    mockList.mockResolvedValue(profiles(1));
    const { container } = render(<ProfileChip />);
    const chip = await waitFor(() => {
      const button = container.querySelector<HTMLButtonElement>("#profile-chip");
      if (!button) throw new Error("not yet");
      return button;
    });
    fireEvent.click(chip);
    expect(container.querySelector("#profile-chip-manage")).toBeNull();
    fireEvent.click(screen.getByRole("menuitem"));
    expect(container.querySelector("#add-open")).not.toBeNull();
  });

  // #767 / PFL-032 — several profiles: the menu switches, and Escape closes it.
  it("switches profile from the menu, and closes on Escape", async () => {
    mockList.mockResolvedValue(profiles(2));
    mockSetInUse.mockResolvedValue({ status: "ok", data: null });
    const { container } = render(<ProfileChip />);
    const chip = await waitFor(() => {
      const button = container.querySelector<HTMLButtonElement>("#profile-chip");
      if (!button) throw new Error("not yet");
      return button;
    });
    fireEvent.click(chip);
    fireEvent.keyDown(container.querySelector("#profile-chip-menu") as Element, { key: "Escape" });
    expect(container.querySelector("#profile-chip-menu")).toBeNull();
    fireEvent.click(chip);
    await act(async () => {
      fireEvent.click(container.querySelector("#profile-chip-choose-p-1") as Element);
    });
    expect(mockSetInUse).toHaveBeenCalledWith("p-1");
    expect(mockNavigate).toHaveBeenCalledWith({ to: "/accounts" });
  });
});
