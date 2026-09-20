import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// The real store module pulls in Tauri APIs and gateways; replace it with a
// minimal zustand store exposing the appVersion slice the hook consumes.
vi.mock("@/lib/store", async () => {
  const { create } = await vi.importActual<typeof import("zustand")>("zustand");
  return { useAppStore: create(() => ({ appVersion: "..." })) };
});

import { useAppStore } from "@/lib/store";
import { getWhatsNewLastSeenVersion } from "@/lib/whatsNewStorage";
import { useWhatsNewDialog } from "./useWhatsNewDialog";

const STORAGE_KEY = "whats_new_last_seen_version";

const CHANGELOG_FIXTURE = `# Changelog

## [0.34.0] - 2026-07-05

### Added

- newest feature

## [0.33.0] - 2026-07-04

### Fixed

- a fix

## [0.32.0] - 2026-07-01

### Added

- older feature
`;

describe("useWhatsNewDialog", () => {
  beforeEach(() => {
    localStorage.clear();
    useAppStore.setState({ appVersion: "..." });
  });

  it("shows nothing and does not seed while the app version is unresolved", () => {
    const { result } = renderHook(() => useWhatsNewDialog(CHANGELOG_FIXTURE));
    expect(result.current.sections).toBeNull();
    expect(getWhatsNewLastSeenVersion()).toBeNull();
  });

  it("shows the current version's section on a fresh start (WNW-030)", () => {
    useAppStore.setState({ appVersion: "0.34.0" });
    const { result } = renderHook(() => useWhatsNewDialog(CHANGELOG_FIXTURE));
    expect(result.current.sections?.map((section) => section.version)).toEqual(["0.34.0"]);
    // The dialog is pending — the stored version only advances on dismiss.
    expect(getWhatsNewLastSeenVersion()).toBeNull();
  });

  it("seeds silently on a fresh start when the current version has no section", () => {
    useAppStore.setState({ appVersion: "0.35.0" });
    const { result } = renderHook(() => useWhatsNewDialog(CHANGELOG_FIXTURE));
    expect(result.current.sections).toBeNull();
    expect(getWhatsNewLastSeenVersion()).toBe("0.35.0");
  });

  it("shows nothing when the stored version matches the current version", () => {
    localStorage.setItem(STORAGE_KEY, "0.34.0");
    useAppStore.setState({ appVersion: "0.34.0" });
    const { result } = renderHook(() => useWhatsNewDialog(CHANGELOG_FIXTURE));
    expect(result.current.sections).toBeNull();
  });

  it("stacks the sections between the stored and current versions on upgrade", () => {
    localStorage.setItem(STORAGE_KEY, "0.32.0");
    useAppStore.setState({ appVersion: "0.34.0" });
    const { result } = renderHook(() => useWhatsNewDialog(CHANGELOG_FIXTURE));
    expect(result.current.sections?.map((section) => section.version)).toEqual([
      "0.34.0",
      "0.33.0",
    ]);
    // The dialog is pending — the stored version only advances on dismiss.
    expect(getWhatsNewLastSeenVersion()).toBe("0.32.0");
  });

  it("reacts when the app version resolves after mount", () => {
    localStorage.setItem(STORAGE_KEY, "0.33.0");
    const { result } = renderHook(() => useWhatsNewDialog(CHANGELOG_FIXTURE));
    expect(result.current.sections).toBeNull();

    act(() => useAppStore.setState({ appVersion: "0.34.0" }));

    expect(result.current.sections?.map((section) => section.version)).toEqual(["0.34.0"]);
  });

  it("seeds the current version silently when the changelog yields no sections", () => {
    localStorage.setItem(STORAGE_KEY, "0.32.0");
    useAppStore.setState({ appVersion: "0.34.0" });
    const { result } = renderHook(() => useWhatsNewDialog("not a changelog"));
    expect(result.current.sections).toBeNull();
    expect(getWhatsNewLastSeenVersion()).toBe("0.34.0");
  });

  it("dismiss persists the current version and hides the dialog", () => {
    localStorage.setItem(STORAGE_KEY, "0.32.0");
    useAppStore.setState({ appVersion: "0.34.0" });
    const { result } = renderHook(() => useWhatsNewDialog(CHANGELOG_FIXTURE));
    expect(result.current.sections).not.toBeNull();

    act(() => result.current.dismiss());

    expect(result.current.sections).toBeNull();
    expect(getWhatsNewLastSeenVersion()).toBe("0.34.0");
  });
});
