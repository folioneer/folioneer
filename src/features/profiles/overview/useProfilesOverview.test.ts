import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useProfilesOverview } from "./useProfilesOverview";

const mockOverview = vi.fn();
const mockSetInUse = vi.fn();
let emit: (type: string) => void = () => {};
vi.mock("../gateway", () => ({
  profileGateway: {
    getProfileOverview: () => mockOverview(),
    setProfileInUse: (...args: unknown[]) => mockSetInUse(...args),
    subscribeToEvents: (callback: (type: string) => void) => {
      emit = callback;
      return Promise.resolve(() => {});
    },
  },
}));
const mockNavigate = vi.fn();
vi.mock("@tanstack/react-router", () => ({ useNavigate: () => mockNavigate }));

const overview = {
  rows: [
    {
      profile: { id: "default-profile", name: null },
      account_count: 2,
      value: 1_000_000_000,
      value_incomplete: false,
      ytd_performance_pct: 6_420_000,
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
};

describe("useProfilesOverview", () => {
  beforeEach(() => {
    mockOverview.mockReset();
    mockSetInUse.mockReset();
    mockNavigate.mockReset();
    mockOverview.mockResolvedValue({ status: "ok", data: overview });
  });

  // PFL-055 — read on mount, and again when a transaction changes a figure.
  it("reads every profile's figures, again when a figure changes", async () => {
    const { result } = renderHook(() => useProfilesOverview());
    await waitFor(() => expect(result.current.overview?.rows).toHaveLength(2));
    act(() => emit("TransactionUpdated"));
    await waitFor(() => expect(mockOverview).toHaveBeenCalledTimes(2));
  });

  // #768 — a click makes another profile the one in use and opens its accounts; the one in
  // use opens its accounts without a choice.
  it("opens a profile's accounts, choosing it when it is not the one in use", async () => {
    mockSetInUse.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useProfilesOverview());
    await waitFor(() => expect(result.current.overview).not.toBeNull());
    await act(async () => {
      await result.current.open("default-profile");
    });
    expect(mockSetInUse).not.toHaveBeenCalled();
    await act(async () => {
      await result.current.open("p-1");
    });
    expect(mockSetInUse).toHaveBeenCalledWith("p-1");
    expect(mockNavigate).toHaveBeenCalledTimes(2);
  });

  // F27 — a refused read is kept as the error to show.
  it("keeps a refused read as the error to show", async () => {
    mockOverview.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });
    const { result } = renderHook(() => useProfilesOverview());
    await waitFor(() => expect(result.current.error).toEqual({ key: "error.DatabaseError" }));
  });
});
