import { create } from "zustand";

export type SnackbarVariant = "success" | "error" | "info";

interface SnackbarState {
  message: string;
  variant: SnackbarVariant;
  isVisible: boolean;
  show: (message: string, variant?: SnackbarVariant) => void;
  hide: () => void;
}

let _timer: ReturnType<typeof setTimeout> | null = null;

export const useSnackbarStore = create<SnackbarState>((set) => ({
  message: "",
  variant: "info",
  isVisible: false,

  show: (message, variant = "info") => {
    if (_timer) clearTimeout(_timer);
    _timer = setTimeout(() => {
      set({ isVisible: false });
      _timer = null;
    }, 4000);
    set({ message, variant, isVisible: true });
  },

  hide: () => {
    if (_timer) clearTimeout(_timer);
    _timer = null;
    set({ isVisible: false });
  },
}));

/** Returns the stable `show` function — import this instead of useSnackbarStore to avoid subscribing to all snackbar state. */
export const useSnackbar = () => useSnackbarStore((s) => s.show);
