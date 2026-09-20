import { useCallback, useEffect, useState } from "react";
import { logger } from "@/lib/logger";

// Theme mode: day = always light, night = always dark, auto = follows OS
export type ThemeMode = "day" | "night" | "auto";

const STORAGE_KEY = "theme-mode";
const TAG = "[useThemeToggle]";
const CYCLE: ThemeMode[] = ["day", "night", "auto"];

function getStoredMode(): ThemeMode {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "day" || stored === "night" || stored === "auto") return stored;
  return "auto";
}

function applyTheme(mode: ThemeMode): void {
  const root = document.documentElement;
  if (mode === "night") {
    root.classList.add("dark");
  } else if (mode === "day") {
    root.classList.remove("dark");
  } else {
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    root.classList.toggle("dark", prefersDark);
  }
}

export function useThemeToggle() {
  const [mode, setMode] = useState<ThemeMode>(() => getStoredMode());

  useEffect(() => {
    logger.info(TAG, "hook mounted");
  }, []);

  // Apply theme whenever mode changes
  useEffect(() => {
    applyTheme(mode);
  }, [mode]);

  // For auto mode: re-apply when OS preference changes
  useEffect(() => {
    if (mode !== "auto") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => {
      document.documentElement.classList.toggle("dark", e.matches);
      logger.info(TAG, "OS theme changed", { dark: e.matches });
    };
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [mode]);

  const cycle = useCallback(() => {
    setMode((prev) => {
      const idx = CYCLE.indexOf(prev);
      return CYCLE[(idx + 1) % CYCLE.length] ?? "day";
    });
  }, []);

  // Persist and log whenever mode changes (kept outside updater to avoid Strict Mode double-fire)
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, mode);
    logger.info(TAG, "theme mode changed", { mode });
  }, [mode]);

  return { mode, cycle };
}
