import type { InputHTMLAttributes } from "react";

interface TextFieldProps extends InputHTMLAttributes<HTMLInputElement> {
  id: string;
  label: string;
  error?: string;
}

/**
 * TextField - M3 Design System Text Input Component
 *
 * Modern text input field with label and optional error message.
 * Inherits all standard HTML input attributes.
 *
 * @example
 * <TextField
 *   id="name"
 *   label="Full Name"
 *   type="text"
 *   placeholder="Enter your name"
 *   error={errors.name}
 * />
 */
export function TextField({ id, label, error, className = "", ...props }: TextFieldProps) {
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="m3-input-label">
        {label}
      </label>
      <input
        id={id}
        className={`m3-input w-full ${error ? "border-m3-error" : ""} ${className}`}
        {...props}
      />
      {error && <p className="text-xs text-m3-error mt-1 ml-1">{error}</p>}
    </div>
  );
}
