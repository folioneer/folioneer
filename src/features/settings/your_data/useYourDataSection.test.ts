import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// 1. Mock the gateway module before importing the hook (test-rules.md § Mocking gateway modules)
vi.mock("../gateway", () => ({
  exportPortfolio: vi.fn(),
  pickExportFolder: vi.fn(),
}));

// 2. Import mocked modules for typed access
import * as gateway from "../gateway";
import { useYourDataSection } from "./useYourDataSection";

const report = {
  written_to: "/home/me/exports/folioneer-export-2026-11-07",
  accounts: 3,
  assets: 12,
  transactions: 418,
  prices: 2960,
};

describe("useYourDataSection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // #509 (the validated mock) — CSV is chosen when the section opens.
  it("opens on CSV, idle", () => {
    const { result } = renderHook(() => useYourDataSection());
    expect(result.current.format).toBe("Csv");
    expect(result.current.outcome).toEqual({ kind: "idle" });
  });

  // #509 (the validated mock) — closing the picker exports nothing.
  it("exports nothing when the picker is closed", async () => {
    vi.mocked(gateway.pickExportFolder).mockResolvedValue(null);
    const { result } = renderHook(() => useYourDataSection());
    await act(async () => {
      await result.current.exportData();
    });
    expect(gateway.exportPortfolio).not.toHaveBeenCalled();
    expect(result.current.outcome).toEqual({ kind: "idle" });
  });

  // #509 (the validated mock), EXP-016 — after an export, where it went and what it holds.
  it("exports in the chosen format and tells where and how much", async () => {
    vi.mocked(gateway.pickExportFolder).mockResolvedValue("/home/me/exports");
    vi.mocked(gateway.exportPortfolio).mockResolvedValue({ status: "ok", data: report });
    const { result } = renderHook(() => useYourDataSection());
    act(() => result.current.setFormat("Json"));
    await act(async () => {
      await result.current.exportData();
    });
    expect(gateway.exportPortfolio).toHaveBeenCalledWith("Json", "/home/me/exports");
    expect(result.current.outcome).toEqual({
      kind: "done",
      message: {
        key: "settings.your_data.done",
        vars: {
          name: "folioneer-export-2026-11-07",
          accounts: 3,
          assets: 12,
          transactions: 418,
          prices: 2960,
        },
      },
    });
  });

  // #509 (the validated mock), EXP-016 — a failed export says that nothing was written, with its reason.
  it("says nothing was written when the export fails", async () => {
    vi.mocked(gateway.pickExportFolder).mockResolvedValue("C:\\exports");
    vi.mocked(gateway.exportPortfolio).mockResolvedValue({
      status: "error",
      error: { code: "WriteFailed" },
    });
    const { result } = renderHook(() => useYourDataSection());
    await act(async () => {
      await result.current.exportData();
    });
    expect(result.current.outcome).toEqual({
      kind: "failed",
      message: { key: "settings.your_data.failed.write_failed" },
    });
  });

  // #509 (the validated mock) — while an export is written, it cannot be started again.
  it("is working while the export is written", async () => {
    vi.mocked(gateway.pickExportFolder).mockResolvedValue("/home/me/exports");
    vi.mocked(gateway.exportPortfolio).mockReturnValue(new Promise(() => {}));
    const { result } = renderHook(() => useYourDataSection());
    await act(async () => {
      void result.current.exportData();
    });
    expect(result.current.outcome).toEqual({ kind: "working" });
  });

  // A thrown call ends as failed, never stuck on working.
  it("ends as failed when the call throws", async () => {
    vi.mocked(gateway.pickExportFolder).mockResolvedValue("/home/me/exports");
    vi.mocked(gateway.exportPortfolio).mockRejectedValue(new Error("bridge gone"));
    const { result } = renderHook(() => useYourDataSection());
    await act(async () => {
      await result.current.exportData();
    });
    expect(result.current.outcome.kind).toBe("failed");
  });

  // A new format clears the line about the last export.
  it("clears the last export's line when the format changes", async () => {
    vi.mocked(gateway.pickExportFolder).mockResolvedValue("/home/me/exports");
    vi.mocked(gateway.exportPortfolio).mockResolvedValue({ status: "ok", data: report });
    const { result } = renderHook(() => useYourDataSection());
    await act(async () => {
      await result.current.exportData();
    });
    act(() => result.current.setFormat("Json"));
    expect(result.current.outcome).toEqual({ kind: "idle" });
  });
});
