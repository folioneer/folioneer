import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useThemeToggle } from "./useThemeToggle";

// Stable mock factory to avoid infinite loops
function makeMockMediaQuery(matches: boolean) {
  const listeners: Array<(e: MediaQueryListEvent) => void> = [];
  return {
    matches,
    addEventListener: vi.fn((_: string, cb: (e: MediaQueryListEvent) => void) => {
      listeners.push(cb);
    }),
    removeEventListener: vi.fn((_: string, cb: (e: MediaQueryListEvent) => void) => {
      const idx = listeners.indexOf(cb);
      if (idx !== -1) listeners.splice(idx, 1);
    }),
    dispatchChange: (newMatches: boolean) => {
      for (const cb of listeners) cb({ matches: newMatches } as MediaQueryListEvent);
    },
    _listeners: listeners,
  };
}

describe("useThemeToggle", () => {
  let mockMq: ReturnType<typeof makeMockMediaQuery>;

  beforeEach(() => {
    localStorage.clear();
    document.documentElement.classList.remove("dark");
    mockMq = makeMockMediaQuery(false);
    vi.spyOn(window, "matchMedia").mockReturnValue(mockMq as unknown as MediaQueryList);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("defaults to auto when localStorage is empty", () => {
    const { result } = renderHook(() => useThemeToggle());
    expect(result.current.mode).toBe("auto");
  });

  it("reads initial mode from localStorage", () => {
    localStorage.setItem("theme-mode", "night");
    const { result } = renderHook(() => useThemeToggle());
    expect(result.current.mode).toBe("night");
  });

  it("falls back to auto for an unknown stored value", () => {
    localStorage.setItem("theme-mode", "invalid");
    const { result } = renderHook(() => useThemeToggle());
    expect(result.current.mode).toBe("auto");
  });

  it("cycles day -> night -> auto -> day", () => {
    localStorage.setItem("theme-mode", "day");
    const { result } = renderHook(() => useThemeToggle());

    act(() => result.current.cycle());
    expect(result.current.mode).toBe("night");

    act(() => result.current.cycle());
    expect(result.current.mode).toBe("auto");

    act(() => result.current.cycle());
    expect(result.current.mode).toBe("day");
  });

  it("persists the new mode to localStorage after cycling", () => {
    localStorage.setItem("theme-mode", "day");
    const { result } = renderHook(() => useThemeToggle());

    act(() => result.current.cycle());
    expect(localStorage.getItem("theme-mode")).toBe("night");
  });

  it("adds dark class in night mode", () => {
    localStorage.setItem("theme-mode", "night");
    renderHook(() => useThemeToggle());
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("removes dark class in day mode", () => {
    document.documentElement.classList.add("dark");
    localStorage.setItem("theme-mode", "day");
    renderHook(() => useThemeToggle());
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("reflects OS preference in auto mode", () => {
    mockMq = makeMockMediaQuery(true);
    vi.spyOn(window, "matchMedia").mockReturnValue(mockMq as unknown as MediaQueryList);
    localStorage.setItem("theme-mode", "auto");
    renderHook(() => useThemeToggle());
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("updates dark class when OS preference changes in auto mode", () => {
    localStorage.setItem("theme-mode", "auto");
    renderHook(() => useThemeToggle());

    act(() => mockMq.dispatchChange(true));
    expect(document.documentElement.classList.contains("dark")).toBe(true);

    act(() => mockMq.dispatchChange(false));
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("removes the OS change listener on unmount when in auto mode", () => {
    localStorage.setItem("theme-mode", "auto");
    const { unmount } = renderHook(() => useThemeToggle());
    unmount();
    expect(mockMq.removeEventListener).toHaveBeenCalledWith("change", expect.any(Function));
  });

  it("does not register OS change listener in non-auto modes", () => {
    localStorage.setItem("theme-mode", "night");
    renderHook(() => useThemeToggle());
    expect(mockMq.addEventListener).not.toHaveBeenCalled();
  });
});
