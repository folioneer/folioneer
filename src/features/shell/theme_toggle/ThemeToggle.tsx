import { Monitor, Moon, Sun } from "lucide-react";
import { type ThemeMode, useThemeToggle } from "./useThemeToggle";

const ICONS: Record<ThemeMode, React.ElementType> = {
  day: Sun,
  night: Moon,
  auto: Monitor,
};

// Label describes the action (next mode), not the current state — per ARIA toggle pattern.
const NEXT_LABELS: Record<ThemeMode, string> = {
  day: "Switch to night mode",
  night: "Switch to auto mode",
  auto: "Switch to day mode",
};

export function ThemeToggle() {
  const { mode, cycle } = useThemeToggle();
  const Icon = ICONS[mode];

  // Raw white tokens are intentional: this button lives exclusively inside the
  // fixed-brand indigo header gradient (--color-header-from/to), which never
  // changes in dark mode. White is always accessible on rich indigo (WCAG AA).
  return (
    <button
      type="button"
      onClick={cycle}
      aria-label={NEXT_LABELS[mode]}
      title={NEXT_LABELS[mode]}
      className="
        flex items-center justify-center
        w-10 h-10 p-0 m-0
        bg-transparent border-none cursor-pointer
        text-white rounded-xl
        transition-colors duration-150
        hover:bg-white/10
        focus-visible:outline-2 focus-visible:outline-white focus-visible:outline-offset-2
      "
    >
      <Icon className="w-5 h-5" />
    </button>
  );
}
