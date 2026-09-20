import { Loader2 } from "lucide-react";
import type { ButtonHTMLAttributes, ReactNode } from "react";

type ButtonVariant = "primary" | "secondary" | "outline" | "ghost" | "danger" | "tonal";
type ButtonSize = "sm" | "md" | "lg";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: ReactNode;
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  icon?: ReactNode;
  fullWidth?: boolean;
}

export function Button({
  children,
  variant = "primary",
  size = "md",
  loading = false,
  disabled = false,
  icon,
  type = "button",
  onClick,
  fullWidth = false,
  className = "",
  ...props
}: ButtonProps) {
  const baseClasses = `
    inline-flex items-center justify-center gap-2
    border rounded-xl
    font-medium tracking-wide
    cursor-pointer
    transition-all duration-150 ease-out
    hover:enabled:shadow-elevation-2
    active:enabled:scale-95
    disabled:opacity-50 disabled:cursor-not-allowed
    ${fullWidth ? "w-full" : ""}
  `;

  const sizeClasses = {
    sm: "px-3 py-1.5 h-8 text-xs",
    md: "px-6 py-2 h-10 text-sm",
    lg: "px-8 py-3 h-12 text-base",
  };

  const variantClasses = {
    primary:
      "bg-m3-primary text-m3-on-primary border-transparent hover:enabled:bg-m3-primary-container active:enabled:bg-m3-primary-container",
    secondary:
      "bg-m3-secondary-container text-m3-on-secondary-container border-m3-secondary-container hover:enabled:bg-m3-secondary-container/90",
    outline:
      "bg-transparent text-m3-on-surface border-m3-outline hover:enabled:bg-m3-surface-variant/30",
    ghost:
      "bg-transparent text-m3-on-surface-variant border-transparent hover:enabled:bg-m3-surface-variant/40",
    danger:
      "bg-m3-error text-m3-on-error border-m3-error hover:enabled:bg-m3-error/90 active:enabled:bg-m3-error",
    tonal:
      "bg-m3-tertiary-container text-m3-on-tertiary-container border-transparent hover:enabled:brightness-110",
  };

  return (
    <button
      type={type}
      className={`${baseClasses} ${sizeClasses[size]} ${variantClasses[variant]} ${className}`}
      disabled={disabled || loading}
      onClick={onClick}
      {...props}
    >
      {loading ? (
        <Loader2 size={size === "sm" ? 14 : size === "md" ? 18 : 20} className="animate-spin" />
      ) : icon ? (
        <span className="flex items-center">{icon}</span>
      ) : null}
      <span>{children}</span>
    </button>
  );
}
