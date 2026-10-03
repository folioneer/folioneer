import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SyncSummary } from "./SyncSummary";

const { mockGetSyncStatus, mockNavigate } = vi.hoisted(() => ({
  mockGetSyncStatus: vi.fn(),
  mockNavigate: vi.fn(),
}));

vi.mock("../gateway", () => ({ getSyncStatus: mockGetSyncStatus }));
vi.mock("@tanstack/react-router", () => ({ useNavigate: () => mockNavigate }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const status = (enabled: boolean, paused: boolean) => ({
  status: "ok",
  data: { enabled, paused },
});

// #010 — the settings keep one line about sync and the way to its page.
describe("SyncSummary", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it.each([
    [false, false, "sync.status_disabled"],
    [true, false, "sync.status_enabled"],
    [true, true, "sync.status_paused"],
  ])("says whether sync is enabled or paused (%s, %s)", async (enabled, paused, expected) => {
    mockGetSyncStatus.mockResolvedValue(status(enabled, paused));
    render(<SyncSummary />);

    await waitFor(() =>
      expect(document.getElementById("settings-sync-state")).toHaveTextContent(expected),
    );
  });

  it("opens the sync page", async () => {
    mockGetSyncStatus.mockResolvedValue(status(true, false));
    render(<SyncSummary />);

    fireEvent.click(screen.getByTestId("settings-open-sync"));

    expect(mockNavigate).toHaveBeenCalledWith({ to: "/sync" });
  });

  // F27 — a status that cannot be read is said, and the way to the page stays.
  it("says so when the status cannot be read", async () => {
    mockGetSyncStatus.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });
    render(<SyncSummary />);

    await waitFor(() =>
      expect(document.getElementById("settings-sync-error")).toHaveTextContent(
        "sync.errors.DatabaseError",
      ),
    );
    expect(document.getElementById("settings-sync-state")).toBeNull();
    expect(screen.getByTestId("settings-open-sync")).toBeInTheDocument();
  });
});
