import {
  type ButtonHTMLAttributes,
  type FocusEvent,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
  useEffect,
  useState,
} from "react";
import { createPortal } from "react-dom";

type IconButtonVariant = "filled" | "outlined" | "tonal" | "ghost" | "danger" | "success" | "error";
type IconButtonShape = "round" | "square";
type IconButtonSize = "sm" | "md" | "lg";

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon: ReactNode;
  variant?: IconButtonVariant;
  shape?: IconButtonShape;
  size?: IconButtonSize;
  "aria-label": string;
  /**
   * #712 — shows the button's name in a small label under it, on hover and on keyboard focus.
   * The label is drawn on the page, outside any scrolling area, so a table never clips it. It
   * repeats the name the button already carries, so assistive technologies skip it; it closes
   * on Escape, on any scroll, and when the button becomes disabled.
   */
  tooltip?: boolean;
}

/** Where the label sits: centred under the button, in the viewport's coordinates. */
interface LabelPlace {
  left: number;
  top: number;
}

export function IconButton({
  icon,
  variant = "ghost",
  shape = "square",
  size = "md",
  className = "",
  tooltip = false,
  onMouseEnter,
  onMouseLeave,
  onFocus,
  onBlur,
  onKeyDown,
  ...props
}: IconButtonProps) {
  const [labelAt, setLabelAt] = useState<LabelPlace | null>(null);
  const showLabel = (target: HTMLButtonElement) => {
    if (!tooltip) return;
    const rect = target.getBoundingClientRect();
    setLabelAt({ left: rect.left + rect.width / 2, top: rect.bottom + 6 });
  };
  const hideLabel = () => setLabelAt(null);
  const shown = labelAt !== null;

  // A label placed from the button's position leaves it as soon as anything scrolls.
  useEffect(() => {
    if (!shown) return;
    const hide = () => setLabelAt(null);
    window.addEventListener("scroll", hide, true);
    return () => window.removeEventListener("scroll", hide, true);
  }, [shown]);

  // A button disabled under the pointer or the focus gets no leave nor blur.
  useEffect(() => {
    if (props.disabled) setLabelAt(null);
  }, [props.disabled]);

  const sizeClasses: Record<IconButtonSize, string> = {
    sm: "h-8 w-8",
    md: "h-10 w-10",
    lg: "h-12 w-12",
  };

  const shapeClasses: Record<IconButtonShape, string> = {
    round: "rounded-full",
    square: "rounded-xl",
  };

  const variantClasses: Record<IconButtonVariant, string> = {
    filled:
      "bg-m3-primary text-m3-on-primary hover:enabled:bg-m3-primary-container hover:enabled:shadow-elevation-2",
    outlined:
      "bg-transparent border border-m3-outline text-m3-primary hover:enabled:bg-m3-surface-variant/30",
    tonal: "bg-m3-tertiary-container text-m3-on-tertiary-container hover:enabled:brightness-110",
    ghost: "bg-transparent text-m3-on-surface-variant hover:enabled:bg-m3-surface-variant/40",
    danger:
      "bg-transparent text-m3-on-surface-variant hover:enabled:text-m3-error hover:enabled:bg-m3-error/10",
    success: "bg-m3-success-container text-m3-on-success-container hover:enabled:brightness-105",
    error: "bg-m3-error-container text-m3-on-error-container hover:enabled:brightness-105",
  };

  return (
    <button
      type="button"
      className={`
        inline-flex items-center justify-center
        transition-all duration-150 ease-out
        active:enabled:scale-95
        disabled:opacity-50 disabled:cursor-not-allowed
        cursor-pointer
        ${sizeClasses[size]}
        ${shapeClasses[shape]}
        ${variantClasses[variant]}
        ${className}
      `}
      {...props}
      onMouseEnter={(event: MouseEvent<HTMLButtonElement>) => {
        showLabel(event.currentTarget);
        onMouseEnter?.(event);
      }}
      onMouseLeave={(event: MouseEvent<HTMLButtonElement>) => {
        hideLabel();
        onMouseLeave?.(event);
      }}
      onFocus={(event: FocusEvent<HTMLButtonElement>) => {
        showLabel(event.currentTarget);
        onFocus?.(event);
      }}
      onBlur={(event: FocusEvent<HTMLButtonElement>) => {
        hideLabel();
        onBlur?.(event);
      }}
      onKeyDown={(event: KeyboardEvent<HTMLButtonElement>) => {
        if (event.key === "Escape") hideLabel();
        onKeyDown?.(event);
      }}
    >
      {icon}
      {labelAt &&
        createPortal(
          <span
            aria-hidden="true"
            data-testid="icon-button-label"
            style={{ left: labelAt.left, top: labelAt.top }}
            className="pointer-events-none fixed z-[60] -translate-x-1/2 whitespace-nowrap rounded px-2 py-1 text-xs bg-m3-on-surface text-m3-surface shadow-elevation-2"
          >
            {props["aria-label"]}
          </span>,
          document.body,
        )}
    </button>
  );
}
