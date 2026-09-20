import { renderHook } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockCheckForUpdate } = vi.hoisted(() => ({
  mockCheckForUpdate: vi.fn(),
}));

vi.mock("@/lib/updateGateway", () => ({
  updateGateway: {
    checkForUpdate: mockCheckForUpdate,
  },
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn() },
}));

const { useAboutPage } = await import("./useAboutPage");

describe("useAboutPage", () => {
  beforeEach(() => {
    mockCheckForUpdate.mockReset();
  });

  it("starts with idle checkStatus", () => {
    const { result } = renderHook(() => useAboutPage());
    expect(result.current.checkStatus).toBe("idle");
  });

  it("sets up_to_date when checkForUpdate returns null", async () => {
    mockCheckForUpdate.mockResolvedValue(null);
    const { result } = renderHook(() => useAboutPage());
    await act(async () => {
      await result.current.handleCheckForUpdate();
    });
    expect(result.current.checkStatus).toBe("up_to_date");
  });

  it("sets idle when checkForUpdate returns an update (banner handles display)", async () => {
    mockCheckForUpdate.mockResolvedValue({ version: "1.2.0" });
    const { result } = renderHook(() => useAboutPage());
    await act(async () => {
      await result.current.handleCheckForUpdate();
    });
    expect(result.current.checkStatus).toBe("idle");
  });

  it("sets error when checkForUpdate throws", async () => {
    mockCheckForUpdate.mockRejectedValue(new Error("network failure"));
    const { result } = renderHook(() => useAboutPage());
    await act(async () => {
      await result.current.handleCheckForUpdate();
    });
    expect(result.current.checkStatus).toBe("error");
  });

  it("does not re-enter while checking (guard)", async () => {
    let resolve: (v: null) => void = () => {};
    mockCheckForUpdate.mockImplementation(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const { result } = renderHook(() => useAboutPage());

    await act(async () => {
      result.current.handleCheckForUpdate();
    });
    expect(result.current.checkStatus).toBe("checking");

    await act(async () => {
      await result.current.handleCheckForUpdate();
    });
    expect(mockCheckForUpdate).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolve(null);
    });
  });
});
