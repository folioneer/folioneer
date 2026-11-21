import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
  }),
}));

import { UpdateBanner } from "./UpdateBanner";
import type { UpdateBannerData } from "./useUpdateBanner";

function data(overrides: Partial<UpdateBannerData>): UpdateBannerData {
  return {
    state: "idle",
    errorKey: "update.error",
    canRetry: true,
    version: "1.2.3",
    progress: 0,
    isRestarting: false,
    handleInstall: vi.fn(),
    handleDismiss: vi.fn(),
    handleRetry: vi.fn(),
    handleRestart: vi.fn(async () => {}),
    ...overrides,
  };
}

describe("UpdateBanner", () => {
  // UPD-002 — nothing is shown while there is nothing to offer.
  it("renders nothing when idle", () => {
    const { container } = render(<UpdateBanner data={data({ state: "idle" })} />);

    expect(container.firstChild).toBeNull();
  });

  // UPD-003 / UPD-005 — the version, Install, and Dismiss with its ×.
  it("offers the version with Install, Dismiss and its close button", () => {
    const view = data({ state: "available" });
    render(<UpdateBanner data={view} />);

    expect(screen.getByText('update.available:{"version":"1.2.3"}')).toBeTruthy();
    fireEvent.click(screen.getByText("update.action_install"));
    fireEvent.click(screen.getByText("update.action_dismiss"));
    fireEvent.click(screen.getByLabelText("action.close"));
    expect(view.handleInstall).toHaveBeenCalledTimes(1);
    expect(view.handleDismiss).toHaveBeenCalledTimes(2);
  });

  // UPD-008 — the progress replaces Install.
  it("shows the progress while downloading, with no Install", () => {
    render(<UpdateBanner data={data({ state: "downloading", progress: 42 })} />);

    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("42");
    expect(screen.getByText('update.progress_label:{"percent":42}')).toBeTruthy();
    expect(screen.queryByText("update.action_install")).toBeNull();
  });

  // UPD-011 / UPD-012 — Restart now, and nothing to dismiss.
  it("offers Restart now and no way to dismiss when ready", () => {
    const view = data({ state: "ready" });
    render(<UpdateBanner data={view} />);

    fireEvent.click(screen.getByText("update.action_restart"));
    expect(view.handleRestart).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("update.action_dismiss")).toBeNull();
    expect(screen.queryByLabelText("action.close")).toBeNull();
  });

  // UPD-013 — the button is held while the installation runs.
  it("disables Restart now while restarting", () => {
    render(<UpdateBanner data={data({ state: "ready", isRestarting: true })} />);

    expect(
      (screen.getByText("update.action_restart").closest("button") as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  // UPD-023 / UPD-024 — a failure offers Retry.
  it("shows the error with Retry", () => {
    const view = data({ state: "error" });
    render(<UpdateBanner data={view} />);

    expect(screen.getByText("update.error")).toBeTruthy();
    fireEvent.click(screen.getByText("update.action_retry"));
    expect(view.handleRetry).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("update.action_dismiss")).toBeNull();
  });

  // #873's mock — a refused access names the refusal and offers Dismiss only.
  it("names a refusal and offers Dismiss only", () => {
    const view = data({
      state: "error",
      canRetry: false,
      errorKey: "update.error_access_refused",
    });
    render(<UpdateBanner data={view} />);

    expect(screen.getByText("update.error_access_refused")).toBeTruthy();
    expect(screen.queryByText("update.action_retry")).toBeNull();
    fireEvent.click(screen.getByText("update.action_dismiss"));
    expect(view.handleDismiss).toHaveBeenCalledTimes(1);
  });
});
