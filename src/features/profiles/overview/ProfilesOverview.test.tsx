import { act, fireEvent, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProfilesOverview } from "./ProfilesOverview";

const mockOverview = vi.fn();
const mockSetInUse = vi.fn();
vi.mock("../gateway", () => ({
  profileGateway: {
    getProfileOverview: () => mockOverview(),
    setProfileInUse: (...args: unknown[]) => mockSetInUse(...args),
    subscribeToEvents: () => Promise.resolve(() => {}),
  },
}));
const mockNavigate = vi.fn();
vi.mock("@tanstack/react-router", () => ({ useNavigate: () => mockNavigate }));

describe("ProfilesOverview", () => {
  beforeEach(() => {
    mockOverview.mockReset();
    mockSetInUse.mockReset();
    mockNavigate.mockReset();
  });

  // PFL-055 / #768 — one row per profile, no total; a click opens that profile's accounts.
  it("lists every profile and opens one on a click", async () => {
    mockOverview.mockResolvedValue({
      status: "ok",
      data: {
        rows: [
          {
            profile: { id: "default-profile", name: null },
            account_count: 1,
            value: 1_000_000_000,
            value_incomplete: true,
            ytd_performance_pct: -1_000_000,
          },
          {
            profile: { id: "p-1", name: "Parent" },
            account_count: 0,
            value: 0,
            value_incomplete: false,
            ytd_performance_pct: null,
          },
        ],
        currency: "EUR",
        in_use: "default-profile",
      },
    });
    mockSetInUse.mockResolvedValue({ status: "ok", data: null });
    const { container } = render(<ProfilesOverview />);
    expect(container.querySelector("#profiles-overview-loading")).not.toBeNull();
    await waitFor(() => expect(container.querySelector("#profiles-overview-table")).not.toBeNull());
    expect(container.querySelectorAll("tbody tr")).toHaveLength(2);
    await act(async () => {
      fireEvent.click(container.querySelector("#profiles-overview-open-p-1") as Element);
    });
    expect(mockSetInUse).toHaveBeenCalledWith("p-1");
    expect(mockNavigate).toHaveBeenCalledWith({ to: "/accounts" });
  });
});
