import type { ButtonHTMLAttributes, ReactNode } from "react";

type IconButtonVariant = "filled" | "outlined" | "tonal" | "ghost" | "danger" | "success" | "error";
type IconButtonShape = "round" | "square";
type IconButtonSize = "sm" | "md" | "lg";

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon: ReactNode;
  variant?: IconButtonVariant;
  shape?: IconButtonShape;
  size?: IconButtonSize;
  "aria-label": string;
}

export function IconButton({
  icon,
  variant = "ghost",
  shape = "square",
  size = "md",
  className = "",
  ...props
}: IconButtonProps) {
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
    >
      {icon}
    </button>
  );
}
